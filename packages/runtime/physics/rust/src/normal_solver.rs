use crate::{
    CoupledNormalSolution, CoupledNormalSystem, DenseLinearSystem, ResponseError,
    solve_contact_complementarity,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalSolvePolicy<'a> {
    pub history: &'a [i16],
    pub inverse_responses: &'a [f32],
    pub gravity_magnitude: f32,
    pub maximum_dimension: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NormalSolveMethod {
    Retained,
    Complementarity,
}

pub(crate) fn solve(
    system: CoupledNormalSystem<'_>,
    policy: NormalSolvePolicy<'_>,
) -> Result<Option<CoupledNormalSolution>, ResponseError> {
    let count = system.right_hand_side.len();
    if count > policy.maximum_dimension
        || count.checked_mul(count) != Some(system.matrix.len())
        || policy.history.len() != count
        || policy.inverse_responses.len() != count
    {
        return Err(ResponseError::InvalidSystemShape);
    }
    if system
        .matrix
        .iter()
        .chain(system.right_hand_side)
        .any(|v| !v.is_finite())
        || !system.scale.is_finite()
        || !system.timestep.is_finite()
        || !policy.gravity_magnitude.is_finite()
        || policy.inverse_responses.iter().any(|v| !v.is_finite())
    {
        return Err(ResponseError::NonFinite);
    }
    if system.timestep <= 0.0 {
        return Err(ResponseError::NonPositiveTimestep);
    }
    if system.scale <= 0.0
        || policy.gravity_magnitude < 0.0
        || policy.inverse_responses.iter().any(|v| *v < 0.0)
    {
        return Err(ResponseError::NonPositiveMass);
    }
    let retained = policy
        .history
        .iter()
        .enumerate()
        .filter_map(|(index, history)| (*history != 0).then_some(index))
        .collect::<Vec<_>>();
    let mut submatrix = Vec::with_capacity(retained.len() * retained.len());
    for row in &retained {
        for column in &retained {
            submatrix.push(system.matrix[row * count + column]);
        }
    }
    let mut dense = DenseLinearSystem::new(
        submatrix,
        retained
            .iter()
            .map(|i| system.right_hand_side[*i])
            .collect(),
        1.0e-9,
        policy.maximum_dimension,
    )?;
    let mut impulses = vec![0.0; count];
    let mut accepted = dense.solve()?;
    if accepted {
        let allowed = f64::from(policy.gravity_magnitude * 0.01_f32);
        for (row, value) in retained.iter().zip(dense.solution()) {
            impulses[*row] = *value;
            if allowed > (*value * system.scale) * f64::from(policy.inverse_responses[*row]) {
                accepted = false;
            }
        }
        for row in 0..count {
            if policy.history[row] != 0 {
                continue;
            }
            let value = crate::complementarity::product_forward(
                &system.matrix[row * count..(row + 1) * count],
                &impulses,
            );
            let slack = (system.right_hand_side[row] * 0.000_009_999_999_747_378_752).abs();
            if value + slack < system.right_hand_side[row] {
                accepted = false;
                break;
            }
        }
    }
    let method = if accepted {
        NormalSolveMethod::Retained
    } else {
        let warm = policy
            .history
            .iter()
            .take_while(|history| **history < 0)
            .count();
        let result = solve_contact_complementarity(
            system.matrix,
            system.right_hand_side,
            warm,
            policy.maximum_dimension,
        )?;
        if !result.solved {
            return Ok(None);
        }
        impulses = result.impulses;
        NormalSolveMethod::Complementarity
    };
    for impulse in impulses.iter_mut().rev() {
        *impulse *= system.scale;
    }
    let history = policy
        .history
        .iter()
        .zip(&impulses)
        .map(|(history, impulse)| {
            if *impulse > 0.0 {
                if *history >= 0 {
                    -1
                } else {
                    history.wrapping_sub(1)
                }
            } else if *impulse == 0.0 {
                0
            } else {
                let value = if *history < 0 {
                    1
                } else {
                    history.wrapping_add(1)
                };
                if value > 9 { 0 } else { value }
            }
        })
        .collect();
    let inverse_step = 1.0_f32 / system.timestep;
    let forces = impulses
        .iter()
        .map(|impulse| {
            ((if *impulse > 0.0 { *impulse } else { 0.0 }) * f64::from(inverse_step)) as f32
        })
        .collect::<Vec<_>>();
    if impulses.iter().any(|v| !v.is_finite()) || forces.iter().any(|v| !v.is_finite()) {
        return Err(ResponseError::NonFinite);
    }
    let active_rows = impulses
        .iter()
        .enumerate()
        .filter_map(|(i, value)| (*value > 0.0).then_some(i))
        .collect();
    Ok(Some(CoupledNormalSolution {
        active_rows,
        impulses,
        forces,
        history,
        method,
    }))
}
