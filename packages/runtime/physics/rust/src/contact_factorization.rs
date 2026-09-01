use crate::ResponseError;

#[derive(Clone, Debug, PartialEq)]
pub struct ContactFactorization {
    transform: Vec<f64>,
    triangular: Vec<f64>,
    dimension: usize,
    capacity: usize,
    tolerance: f64,
    usable: bool,
}

impl ContactFactorization {
    pub fn new(capacity: usize, tolerance: f64) -> Result<Self, ResponseError> {
        let entries = capacity
            .checked_mul(capacity)
            .ok_or(ResponseError::InvalidSystemShape)?;
        if !tolerance.is_finite() {
            return Err(ResponseError::NonFinite);
        }
        if tolerance <= 0.0 {
            return Err(ResponseError::InvalidSystemShape);
        }
        Ok(Self {
            transform: vec![0.0; entries],
            triangular: vec![0.0; entries],
            dimension: 0,
            capacity,
            tolerance,
            usable: true,
        })
    }
    pub fn dimension(&self) -> usize {
        self.dimension
    }
    pub fn is_usable(&self) -> bool {
        self.usable
    }
    pub fn transform_row(&self, row: usize) -> Option<&[f64]> {
        self.row(&self.transform, row)
    }
    pub fn triangular_row(&self, row: usize) -> Option<&[f64]> {
        self.row(&self.triangular, row)
    }
    fn row<'a>(&self, values: &'a [f64], row: usize) -> Option<&'a [f64]> {
        (row < self.dimension)
            .then(|| &values[row * self.capacity..row * self.capacity + self.dimension])
    }

    pub fn factor(
        &mut self,
        coefficients: &[f64],
        dimension: usize,
    ) -> Result<bool, ResponseError> {
        if dimension > self.capacity || dimension.checked_mul(dimension) != Some(coefficients.len())
        {
            return Err(ResponseError::InvalidSystemShape);
        }
        if coefficients.iter().any(|v| !v.is_finite()) {
            return Err(ResponseError::NonFinite);
        }
        let mut next = self.clone();
        next.dimension = dimension;
        for row in 0..dimension {
            for column in 0..dimension {
                next.triangular[row * next.capacity + column] =
                    coefficients[row * dimension + column];
                next.transform[row * next.capacity + column] =
                    if row == column { 1.0 } else { 0.0 };
            }
        }
        next.usable = true;
        for column in 0..dimension {
            next.pivot(column);
            if !next.normalize(column) {
                next.usable = false;
                break;
            }
            for row in ((column + 1)..dimension).rev() {
                let value = next.triangular[row * next.capacity + column];
                if value != 0.0 {
                    next.eliminate(column, row, value);
                }
            }
        }
        self.commit(next)
    }

    pub fn solve(&self, rhs: &[f64]) -> Result<Vec<f64>, ResponseError> {
        if rhs.len() != self.dimension {
            return Err(ResponseError::InvalidSystemShape);
        }
        if !self.usable {
            return Err(ResponseError::SingularFactorization);
        }
        if rhs.iter().any(|v| !v.is_finite()) {
            return Err(ResponseError::NonFinite);
        }
        let mut output = self.transform_vector(rhs);
        for row in (0..self.dimension).rev() {
            let mut sum = 0.0;
            for column in ((row + 1)..self.dimension).rev() {
                sum += self.triangular[row * self.capacity + column] * output[column];
            }
            output[row] -= sum;
        }
        if output.iter().any(|v| !v.is_finite()) {
            return Err(ResponseError::NonFinite);
        }
        Ok(output)
    }

    /// Append one equation and variable. The last row includes its diagonal; the column excludes it.
    pub fn append(&mut self, row: &[f64], column: &[f64]) -> Result<bool, ResponseError> {
        let n = self.dimension;
        if n == self.capacity || row.len() != n + 1 || column.len() != n {
            return Err(ResponseError::InvalidSystemShape);
        }
        if !self.usable {
            return Err(ResponseError::SingularFactorization);
        }
        if row.iter().chain(column).any(|v| !v.is_finite()) {
            return Err(ResponseError::NonFinite);
        }
        let mut next = self.clone();
        let transformed = next.transform_vector(column);
        for (index, value) in transformed.into_iter().enumerate() {
            next.triangular[index * next.capacity + n] = value;
            next.transform[index * next.capacity + n] = 0.0;
            next.transform[n * next.capacity + index] = 0.0;
        }
        next.transform[n * next.capacity + n] = 1.0;
        next.triangular[n * next.capacity..n * next.capacity + n + 1].copy_from_slice(row);
        next.dimension += 1;
        for index in 0..n {
            let value = next.triangular[n * next.capacity + index];
            next.eliminate(index, n, value);
        }
        next.usable = next.normalize(n);
        self.commit(next)
    }

    /// The last variable fills the removed variable's slot.
    pub fn remove(&mut self, variable: usize) -> Result<bool, ResponseError> {
        if variable >= self.dimension {
            return Err(ResponseError::InvalidSystemShape);
        }
        if !self.usable {
            return Err(ResponseError::SingularFactorization);
        }
        let mut next = self.clone();
        let last = next.dimension - 1;
        for row in 0..next.dimension {
            next.transform
                .swap(row * next.capacity + variable, row * next.capacity + last);
            next.triangular
                .swap(row * next.capacity + variable, row * next.capacity + last);
        }
        for row in ((variable + 1)..last).rev() {
            let value = next.triangular[row * next.capacity + variable];
            if value != 0.0 {
                next.add_transform_row(last, row, -value);
            }
            next.triangular[row * next.capacity + variable] = 0.0;
        }
        if !next.normalize(variable) {
            next.add_transform_row(last, variable, 1.0);
            next.triangular[variable * next.capacity + variable] = 1.0;
        }
        for column in variable..last {
            let value = next.triangular[last * next.capacity + column];
            if value != 0.0 {
                next.eliminate(column, last, value);
            }
        }
        let diagonal = next.transform[last * next.capacity + last];
        if diagonal.abs() < next.tolerance {
            next.dimension -= 1;
            next.usable = false;
            return self.commit(next);
        }
        let reciprocal = 1.0 / diagonal;
        for column in 0..next.dimension {
            next.transform[last * next.capacity + column] *= reciprocal;
        }
        next.transform[last * next.capacity + last] = 1.0;
        for row in (0..last).rev() {
            let value = next.transform[row * next.capacity + last];
            if value != 0.0 {
                next.add_transform_row(last, row, -value);
                next.transform[row * next.capacity + last] = 0.0;
            }
        }
        next.dimension -= 1;
        self.commit(next)
    }

    fn commit(&mut self, next: Self) -> Result<bool, ResponseError> {
        if next
            .transform
            .iter()
            .chain(&next.triangular)
            .any(|v| !v.is_finite())
        {
            return Err(ResponseError::NonFinite);
        }
        let usable = next.usable;
        *self = next;
        Ok(usable)
    }
    fn pivot(&mut self, column: usize) {
        let mut selected = column;
        let mut largest = self.triangular[column * self.capacity + column].abs();
        for row in ((column + 1)..self.dimension).rev() {
            let value = self.triangular[row * self.capacity + column].abs();
            if value > largest {
                largest = value;
                selected = row;
            }
        }
        if selected != column {
            for offset in 0..self.dimension {
                self.transform.swap(
                    column * self.capacity + offset,
                    selected * self.capacity + offset,
                );
            }
            for offset in column..self.dimension {
                self.triangular.swap(
                    column * self.capacity + offset,
                    selected * self.capacity + offset,
                );
            }
        }
    }
    fn normalize(&mut self, row: usize) -> bool {
        let diagonal = self.triangular[row * self.capacity + row];
        if diagonal.abs() < self.tolerance {
            return false;
        }
        let reciprocal = 1.0 / diagonal;
        for column in 0..self.dimension {
            self.transform[row * self.capacity + column] *= reciprocal;
        }
        for column in row + 1..self.dimension {
            self.triangular[row * self.capacity + column] *= reciprocal;
        }
        self.triangular[row * self.capacity + row] = 1.0;
        true
    }
    fn eliminate(&mut self, source: usize, destination: usize, value: f64) {
        let factor = -value;
        for column in source + 1..self.dimension {
            let updated = factor * self.triangular[source * self.capacity + column]
                + self.triangular[destination * self.capacity + column];
            self.triangular[destination * self.capacity + column] = updated;
        }
        self.add_transform_row(source, destination, factor);
        self.triangular[destination * self.capacity + source] = 0.0;
    }
    fn add_transform_row(&mut self, source: usize, destination: usize, factor: f64) {
        for column in 0..self.dimension {
            let updated = factor * self.transform[source * self.capacity + column]
                + self.transform[destination * self.capacity + column];
            self.transform[destination * self.capacity + column] = updated;
        }
    }
    fn transform_vector(&self, input: &[f64]) -> Vec<f64> {
        let mut output = vec![0.0; self.dimension];
        for row in (0..self.dimension).rev() {
            let mut value = 0.0;
            for column in (0..self.dimension).rev() {
                value += self.transform[row * self.capacity + column] * input[column];
            }
            output[row] = value;
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn append_and_last_slot_removal_retain_a_solvable_factor() {
        let mut factor = ContactFactorization::new(3, 1.0e-6).unwrap();
        assert!(factor.factor(&[2.0, 0.0, 0.0, 4.0], 2).unwrap());
        assert_eq!(factor.solve(&[4.0, 12.0]).unwrap(), [2.0, 3.0]);
        assert!(factor.append(&[0.0, 0.0, 8.0], &[0.0, 0.0]).unwrap());
        assert_eq!(factor.solve(&[4.0, 12.0, 32.0]).unwrap(), [2.0, 3.0, 4.0]);
        assert!(factor.remove(0).unwrap());
        assert_eq!(factor.solve(&[16.0, 12.0]).unwrap(), [2.0, 3.0]);
    }
    #[test]
    fn rejected_mutations_leave_the_existing_factors_unchanged() {
        let mut factor = ContactFactorization::new(1, 1.0e-6).unwrap();
        assert!(factor.factor(&[2.0], 1).unwrap());
        let before = factor.clone();
        assert_eq!(factor.factor(&[f64::NAN], 1), Err(ResponseError::NonFinite));
        assert_eq!(factor, before);
        assert_eq!(
            factor.append(&[1.0, 2.0], &[0.0]),
            Err(ResponseError::InvalidSystemShape)
        );
        assert_eq!(factor, before);
        assert_eq!(factor.remove(1), Err(ResponseError::InvalidSystemShape));
        assert_eq!(factor, before);
        assert!(!factor.factor(&[0.0], 1).unwrap());
        assert_eq!(
            factor.solve(&[1.0]),
            Err(ResponseError::SingularFactorization)
        );
    }
}
