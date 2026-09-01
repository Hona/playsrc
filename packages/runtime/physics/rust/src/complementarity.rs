use crate::{ContactFactorization, DenseLinearSystem, ResponseError};

const EPSILON: f64 = 1.000_000_011_686_097_4e-7;
const FACTOR_EPSILON: f64 = 1.000_000_011_686_097_4e-6;
const CHECK_EPSILON: f64 = 0.000_100_000_001_168_609_74;
const MAXIMUM_STEP: f64 = 10_000.0;

#[derive(Clone, Debug, PartialEq)]
pub struct ComplementaritySolution {
    pub solved: bool,
    pub impulses: Vec<f64>,
    pub residuals: Vec<f64>,
    pub order: Vec<usize>,
    pub active: usize,
    pub processed: usize,
    pub iterations: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FactorState {
    Ready,
    Retry,
    Unavailable,
}
struct Direction {
    force: Vec<f64>,
    acceleration: Vec<f64>,
}

struct Search<'a> {
    matrix: &'a [f64],
    rhs: &'a [f64],
    x: Vec<f64>,
    q: Vec<f64>,
    order: Vec<usize>,
    active: usize,
    processed: usize,
    factor: ContactFactorization,
    factor_state: FactorState,
    permutation_counters: [usize; 4],
    iterations: u32,
}

pub fn solve_contact_complementarity(
    matrix: &[f64],
    rhs: &[f64],
    initial_active: usize,
    maximum_dimension: usize,
) -> Result<ComplementaritySolution, ResponseError> {
    let n = rhs.len();
    if n > maximum_dimension || initial_active > n || n.checked_mul(n) != Some(matrix.len()) {
        return Err(ResponseError::InvalidSystemShape);
    }
    if matrix.iter().chain(rhs).any(|v| !v.is_finite()) {
        return Err(ResponseError::NonFinite);
    }
    let mut search = Search {
        matrix,
        rhs,
        x: vec![0.0; n],
        q: rhs.iter().map(|v| -v).collect(),
        order: (0..n).collect(),
        active: initial_active,
        processed: initial_active,
        factor: ContactFactorization::new(n, FACTOR_EPSILON)?,
        factor_state: FactorState::Ready,
        permutation_counters: [0; 4],
        iterations: 0,
    };
    search.initialize()?;
    let solved = search.run()?;
    if search.x.iter().chain(&search.q).any(|v| !v.is_finite()) {
        return Err(ResponseError::NonFinite);
    }
    Ok(ComplementaritySolution {
        solved,
        impulses: search.x,
        residuals: search.q,
        order: search.order,
        active: search.active,
        processed: search.processed,
        iterations: search.iterations,
    })
}

impl Search<'_> {
    fn n(&self) -> usize {
        self.rhs.len()
    }
    fn active_matrix(&self) -> Vec<f64> {
        let mut result = Vec::with_capacity(self.active * self.active);
        for row in &self.order[..self.active] {
            for column in &self.order[..self.active] {
                result.push(self.matrix[row * self.n() + column]);
            }
        }
        result
    }
    fn active_rhs(&self) -> Vec<f64> {
        self.order[..self.active]
            .iter()
            .map(|i| self.rhs[*i])
            .collect()
    }
    fn prepare_factor(&mut self) -> Result<bool, ResponseError> {
        self.factor.factor(&self.active_matrix(), self.active)
    }
    fn refresh_values(&mut self, values: &[f64]) {
        self.x.fill(0.0);
        for (variable, value) in self.order[..self.active].iter().zip(values) {
            self.x[*variable] = *value;
        }
        for row in 0..self.n() {
            self.q[row] =
                product_forward(&self.matrix[row * self.n()..(row + 1) * self.n()], &self.x)
                    - self.rhs[row];
        }
        for variable in &self.order[..self.active] {
            self.q[*variable] = 0.0;
        }
    }
    fn initialize(&mut self) -> Result<(), ResponseError> {
        if !self.prepare_factor()? {
            self.active = 0;
            self.processed = 0;
            self.factor.factor(&[], 0)?;
            return Ok(());
        }
        loop {
            let values = self.factor.solve(&self.active_rhs())?;
            self.refresh_values(&values);
            let Some(index) = (0..self.active)
                .rev()
                .find(|index| self.x[self.order[*index]] < 0.0)
            else {
                return Ok(());
            };
            self.x[self.order[index]] = 0.0;
            self.processed -= 1;
            self.active -= 1;
            self.order.swap(index, self.active);
            if !self.factor.remove(index)? {
                self.factor_state = FactorState::Unavailable;
                self.x.fill(0.0);
                self.q.fill(0.0);
                self.active = 0;
                self.processed = 0;
                return Ok(());
            }
        }
    }
    fn numerical_check(&self) -> bool {
        let mut recalculated = vec![0.0; self.n()];
        for (row, result) in recalculated.iter_mut().enumerate() {
            let mut sum = 0.0;
            for column in (0..self.n()).rev() {
                sum += self.matrix[row * self.n() + column] * self.x[column];
            }
            *result = sum - self.rhs[row];
        }
        self.order[..self.active]
            .iter()
            .all(|i| recalculated[*i].abs() <= CHECK_EPSILON)
            && self.order[self.active..]
                .iter()
                .all(|i| (recalculated[*i] - self.q[*i]).abs() <= CHECK_EPSILON)
    }
    fn permute(&mut self) {
        if self.active < 2 {
            return;
        }
        self.permutation_counters[0] = (self.permutation_counters[0] + 1) % self.active;
        self.permutation_counters[1] = (self.permutation_counters[1] + 2) % self.active;
        self.order
            .swap(self.permutation_counters[0], self.permutation_counters[1]);
        self.permutation_counters[2] += 1;
        self.permutation_counters[3] += 2;
        let remaining = self.n().saturating_sub(self.processed + 1);
        if remaining < 2 {
            return;
        }
        self.permutation_counters[2] %= remaining;
        self.permutation_counters[3] %= remaining;
        self.order.swap(
            self.processed + 1 + self.permutation_counters[2],
            self.processed + 1 + self.permutation_counters[3],
        );
    }
    fn rebuild(&mut self) -> Result<bool, ResponseError> {
        loop {
            self.permute();
            let values = if self.prepare_factor()? {
                self.factor_state = FactorState::Ready;
                self.factor.solve(&self.active_rhs())?
            } else {
                self.factor_state = FactorState::Unavailable;
                for index in 1..self.active {
                    let mut at = index;
                    while at > 0 && self.x[self.order[at]] > self.x[self.order[at - 1]] {
                        self.order.swap(at, at - 1);
                        at -= 1;
                    }
                }
                self.permute();
                let mut dense = DenseLinearSystem::new(
                    self.active_matrix(),
                    self.active_rhs(),
                    EPSILON,
                    self.n(),
                )?;
                if !dense.solve()? {
                    return Ok(false);
                }
                dense.solution().to_vec()
            };
            self.refresh_values(&values);
            let mut removed_negative = false;
            let mut index = 0;
            while index < self.active {
                let variable = self.order[index];
                let value = self.x[variable];
                if value >= EPSILON {
                    index += 1;
                    continue;
                }
                if value > -EPSILON {
                    self.x[variable] = 0.0;
                    self.order.swap(index, self.active - 1);
                } else {
                    let removed = self.order.remove(index);
                    self.order.push(removed);
                    self.processed -= 1;
                    removed_negative = true;
                }
                self.active -= 1;
                self.factor_state = FactorState::Retry;
            }
            index = self.active;
            while index < self.processed {
                if self.q[self.order[index]] < 0.0 {
                    let removed = self.order.remove(index);
                    self.order.push(removed);
                    self.processed -= 1;
                } else {
                    index += 1;
                }
            }
            if !removed_negative {
                return Ok(true);
            }
        }
    }
    fn remove_active(&mut self, index: usize) -> Result<(), ResponseError> {
        self.x[self.order[index]] = 0.0;
        self.active -= 1;
        self.order.swap(index, self.active);
        if self.factor_state == FactorState::Ready && !self.factor.remove(index)? {
            self.factor_state = FactorState::Unavailable;
        }
        Ok(())
    }
    fn drop_small_active(&mut self) -> Result<(), ResponseError> {
        let mut index = 0;
        while index < self.active {
            if self.x[self.order[index]] < EPSILON {
                self.remove_active(index)?;
            } else {
                index += 1;
            }
        }
        Ok(())
    }
    fn append_active(&mut self, index: usize, processed: bool) -> Result<(), ResponseError> {
        let variable = self.order[index];
        self.q[variable] = 0.0;
        if processed && self.x[variable] < 0.0 {
            self.x[variable] = 0.0;
        }
        self.order.swap(index, self.active);
        self.active += 1;
        if processed {
            self.processed += 1;
        }
        if self.processed == self.n() && processed {
            return Ok(());
        }
        if self.factor_state == FactorState::Ready {
            let row = self.order[..self.active]
                .iter()
                .map(|other| self.matrix[variable * self.n() + other])
                .collect::<Vec<_>>();
            let column = self.order[..self.active - 1]
                .iter()
                .map(|other| self.matrix[other * self.n() + variable])
                .collect::<Vec<_>>();
            if !self.factor.append(&row, &column)? {
                self.factor_state = FactorState::Unavailable;
            }
        }
        Ok(())
    }
    fn direction(&self) -> Result<Option<Direction>, ResponseError> {
        let current = self.order[self.processed];
        let mut force = vec![0.0; self.n()];
        let mut acceleration = vec![0.0; self.n()];
        if self.active > 0 {
            let rhs = self.order[..self.active]
                .iter()
                .map(|i| -self.matrix[i * self.n() + current])
                .collect::<Vec<_>>();
            let values = if self.factor_state == FactorState::Ready {
                self.factor.solve(&rhs)?
            } else {
                let mut dense =
                    DenseLinearSystem::new(self.active_matrix(), rhs, EPSILON, self.n())?;
                if !dense.solve()? {
                    return Ok(None);
                }
                dense.solution().to_vec()
            };
            for (variable, value) in self.order[..self.active].iter().zip(values) {
                force[*variable] = value;
            }
        }
        force[current] = 1.0;
        for row in &self.order[self.active..] {
            let mut sum = 0.0;
            for column in &self.order[..self.active] {
                sum += force[*column] * self.matrix[row * self.n() + column];
            }
            acceleration[*row] = sum + self.matrix[row * self.n() + current];
        }
        Ok(Some(Direction {
            force,
            acceleration,
        }))
    }
    fn choose_step(&self, force: &[f64], acceleration: &[f64]) -> Option<(f64, usize)> {
        let current = self.order[self.processed];
        let mut selected = if -self.q[current] < acceleration[current] * MAXIMUM_STEP {
            Some(self.processed)
        } else {
            None
        };
        let mut distance = if selected.is_some() {
            (-1.0 / acceleration[current]) * self.q[current]
        } else {
            1.0e101
        };
        for (index, variable) in self.order[..self.active].iter().enumerate() {
            if force[*variable] >= -EPSILON {
                continue;
            }
            let candidate = (-1.0 / force[*variable]) * self.x[*variable];
            if candidate.abs() < EPSILON && self.x[*variable] < EPSILON {
                return Some((candidate, index));
            }
            if candidate < distance + EPSILON {
                distance = candidate;
                selected = Some(index);
            }
        }
        for index in self.active..self.processed {
            let variable = self.order[index];
            if acceleration[variable] >= -EPSILON {
                continue;
            }
            let candidate = (-1.0 / acceleration[variable]) * self.q[variable];
            if candidate < distance - EPSILON {
                distance = candidate;
                selected = Some(index);
            }
        }
        selected.map(|index| (distance.max(0.0), index))
    }
    fn apply_step(&mut self, distance: f64, force: &[f64], acceleration: &[f64]) {
        for variable in 0..self.n() {
            let residual = distance * acceleration[variable] + self.q[variable];
            let impulse = distance * force[variable] + self.x[variable];
            self.q[variable] = residual;
            self.x[variable] = impulse;
        }
        for variable in &self.order[self.active..self.processed] {
            if self.q[*variable] < 0.0 {
                self.q[*variable] = 0.0;
            }
        }
        for variable in &self.order[..self.active] {
            if self.x[*variable] < 0.0 {
                self.x[*variable] = 0.0;
            }
        }
    }
    fn run(&mut self) -> Result<bool, ResponseError> {
        let mut check_in = 7;
        let mut small_steps = 0;
        let mut changed = false;
        let mut resume_after_rebuild = false;
        loop {
            if resume_after_rebuild {
                resume_after_rebuild = false;
            } else {
                self.iterations += u32::from(changed);
                if self.iterations > 250 {
                    return Ok(false);
                }
                if check_in == 0 {
                    if !self.numerical_check() {
                        self.iterations += 1;
                        if self.iterations > 250 || !self.rebuild()? {
                            return Ok(false);
                        }
                        small_steps = 0;
                    }
                    check_in = 7;
                } else {
                    check_in -= 1;
                }
            }
            if self.processed >= self.n() {
                return Ok(true);
            }
            if self.factor_state != FactorState::Ready {
                if self.factor_state == FactorState::Retry && changed {
                    self.permute();
                    if self.prepare_factor()? {
                        self.factor_state = FactorState::Ready;
                    }
                } else {
                    self.factor_state = FactorState::Retry;
                }
            }
            let current = self.order[self.processed];
            let residual = self.q[current];
            if residual.abs() < EPSILON && self.x[current].abs() >= EPSILON {
                self.drop_small_active()?;
                self.append_active(self.processed, true)?;
                if self.processed == self.n() {
                    check_in = 0;
                }
                continue;
            }
            if residual >= 0.0 || residual.abs() < EPSILON {
                self.x[current] = 0.0;
                if self.q[current] < 0.0 {
                    self.q[current] = 0.0;
                }
                self.processed += 1;
                changed = false;
                if self.processed == self.n() {
                    check_in = 0;
                }
                continue;
            }
            let direction = self.direction()?;
            let selected = direction
                .as_ref()
                .and_then(|direction| self.choose_step(&direction.force, &direction.acceleration));
            if direction.is_some() && selected.is_none() {
                return Ok(false);
            }
            let rebuild = if let Some((step, _)) = selected {
                if step < EPSILON {
                    small_steps += 1;
                } else {
                    small_steps = 0;
                }
                step > MAXIMUM_STEP || small_steps > self.n() / 2 + 2
            } else {
                true
            };
            if rebuild {
                self.iterations += 1;
                if self.iterations > 250 || !self.rebuild()? {
                    return Ok(false);
                }
                small_steps = 0;
                check_in = 7;
                resume_after_rebuild = true;
                continue;
            }
            let (distance, index) = selected.unwrap();
            let direction = direction.unwrap();
            self.apply_step(distance, &direction.force, &direction.acceleration);
            changed = true;
            if index < self.active {
                self.remove_active(index)?;
            } else if index < self.processed {
                if distance > EPSILON {
                    self.drop_small_active()?;
                }
                self.append_active(index, false)?;
            } else {
                self.drop_small_active()?;
                self.append_active(self.processed, true)?;
                if self.processed == self.n() {
                    check_in = 0;
                }
            }
        }
    }
}

pub(crate) fn product_forward(row: &[f64], vector: &[f64]) -> f64 {
    let whole = row.len() / 4 * 4;
    let mut lanes = [0.0; 4];
    for chunk in (0..whole).step_by(4) {
        for lane in 0..4 {
            lanes[lane] += row[chunk + lane] * vector[chunk + lane];
        }
    }
    let mut value = (lanes[0] + lanes[2]) + (lanes[1] + lanes[3]);
    for index in whole..row.len() {
        value += row[index] * vector[index];
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cold_and_warm_two_contact_solves_keep_both_active_rows() {
        for active in 0..=2 {
            let result =
                solve_contact_complementarity(&[2.0, 1.0, 1.0, 2.0], &[3.0, 3.0], active, 2)
                    .unwrap();
            assert!(result.solved);
            assert_eq!(result.impulses, vec![1.0, 1.0]);
            assert_eq!(result.active, 2);
            assert_eq!(result.processed, 2);
        }
    }
    #[test]
    fn a_separating_contact_is_inactive_and_invalid_dimensions_are_rejected() {
        let result =
            solve_contact_complementarity(&[1.0, 0.0, 0.0, 1.0], &[-1.0, 2.0], 0, 2).unwrap();
        assert!(result.solved);
        assert_eq!(result.impulses, vec![0.0, 2.0]);
        assert_eq!(result.residuals, vec![1.0, 0.0]);
        assert_eq!(
            solve_contact_complementarity(&[1.0], &[1.0], 2, 1),
            Err(ResponseError::InvalidSystemShape)
        );
        assert_eq!(
            solve_contact_complementarity(&[f64::NAN], &[1.0], 0, 1),
            Err(ResponseError::NonFinite)
        );
    }
}
