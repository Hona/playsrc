use crate::ResponseError;

#[derive(Clone, Debug, PartialEq)]
pub struct DenseLinearSystem {
    coefficients: Vec<f64>,
    right_hand_side: Vec<f64>,
    solution: Vec<f64>,
    tolerance: f64,
}

impl DenseLinearSystem {
    pub fn new(
        coefficients: Vec<f64>,
        right_hand_side: Vec<f64>,
        tolerance: f64,
        maximum_dimension: usize,
    ) -> Result<Self, ResponseError> {
        let count = right_hand_side.len();
        if count > maximum_dimension || count.checked_mul(count) != Some(coefficients.len()) {
            return Err(ResponseError::InvalidSystemShape);
        }
        if !tolerance.is_finite()
            || coefficients
                .iter()
                .chain(&right_hand_side)
                .any(|v| !v.is_finite())
        {
            return Err(ResponseError::NonFinite);
        }
        if tolerance <= 0.0 {
            return Err(ResponseError::InvalidSystemShape);
        }
        Ok(Self {
            coefficients,
            right_hand_side,
            solution: vec![0.0; count],
            tolerance,
        })
    }
    pub fn coefficients(&self) -> &[f64] {
        &self.coefficients
    }
    pub fn right_hand_side(&self) -> &[f64] {
        &self.right_hand_side
    }
    pub fn solution(&self) -> &[f64] {
        &self.solution
    }

    /// A singular inconsistent system is a completed solve returning false with a zero solution.
    pub fn solve(&mut self) -> Result<bool, ResponseError> {
        let mut next = self.clone();
        let solved = next.eliminate();
        if next
            .coefficients
            .iter()
            .chain(&next.right_hand_side)
            .chain(&next.solution)
            .any(|v| !v.is_finite())
        {
            return Err(ResponseError::NonFinite);
        }
        *self = next;
        Ok(solved)
    }
    fn eliminate(&mut self) -> bool {
        let count = self.right_hand_side.len();
        for column in 0..count {
            let mut selected = column;
            let mut largest = self.coefficients[column * count + column].abs();
            for row in ((column + 1)..count).rev() {
                let magnitude = self.coefficients[row * count + column].abs();
                if magnitude > largest {
                    selected = row;
                    largest = magnitude;
                }
            }
            if selected != column {
                for offset in 0..count {
                    self.coefficients
                        .swap(column * count + offset, selected * count + offset);
                }
                self.right_hand_side.swap(column, selected);
            }
            let diagonal = self.coefficients[column * count + column];
            if diagonal.abs() < self.tolerance {
                continue;
            }
            let factor = -1.0 / diagonal;
            for row in column + 1..count {
                let entry = self.coefficients[row * count + column];
                if entry.abs() <= self.tolerance {
                    continue;
                }
                let factor = entry * factor;
                for offset in column..count {
                    let updated = factor * self.coefficients[column * count + offset]
                        + self.coefficients[row * count + offset];
                    self.coefficients[row * count + offset] = updated;
                }
                let updated = factor * self.right_hand_side[column] + self.right_hand_side[row];
                self.right_hand_side[row] = updated;
            }
        }
        for row in (0..count).rev() {
            let mut value = self.right_hand_side[row];
            for column in (row + 1..count).rev() {
                value -= self.right_hand_side[column] * self.coefficients[row * count + column];
            }
            let diagonal = self.coefficients[row * count + row];
            if diagonal.abs() < self.tolerance {
                if value.abs() >= self.tolerance * 1000.0 {
                    self.solution.fill(0.0);
                    return false;
                }
                value = 0.0;
            } else {
                value /= diagonal;
            }
            self.right_hand_side[row] = value;
        }
        self.solution.copy_from_slice(&self.right_hand_side);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pivoting_and_singular_results_preserve_complete_scratch() {
        let mut system =
            DenseLinearSystem::new(vec![0.0, 2.0, 3.0, 4.0], vec![4.0, 11.0], 1.0e-9, 2).unwrap();
        assert!(system.solve().unwrap());
        assert_eq!(system.solution(), &[1.0, 2.0]);
        let mut singular =
            DenseLinearSystem::new(vec![1.0, 1.0, 1.0, 1.0], vec![1.0, 2.0], 1.0e-9, 2).unwrap();
        assert!(!singular.solve().unwrap());
        assert_eq!(singular.solution(), &[0.0, 0.0]);
    }
    #[test]
    fn empty_system_is_a_successful_empty_solve_and_invalid_inputs_are_rejected() {
        let mut empty = DenseLinearSystem::new(vec![], vec![], 1.0e-9, 0).unwrap();
        assert!(empty.solve().unwrap());
        assert_eq!(
            DenseLinearSystem::new(vec![1.0], vec![1.0], 1.0e-9, 0),
            Err(ResponseError::InvalidSystemShape)
        );
        let mut overflow = DenseLinearSystem::new(
            vec![1.0, 1.0, 1.0, -1.0],
            vec![f64::MAX, -f64::MAX],
            1.0e-9,
            2,
        )
        .unwrap();
        let before = overflow.clone();
        assert_eq!(overflow.solve(), Err(ResponseError::NonFinite));
        assert_eq!(overflow, before);
    }
}
