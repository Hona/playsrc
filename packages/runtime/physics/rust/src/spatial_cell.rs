use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SpatialCell {
    pub origin: [i32; 3],
    pub raster_exponent: i32,
    pub size_exponent: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FittedSpatialCell {
    pub cell: SpatialCell,
    pub radius: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialCellError {
    NonFinite,
    NonPositiveRadius,
    UnsupportedScale,
    CoordinateRange,
}

impl fmt::Display for SpatialCellError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NonFinite => "spatial cell input contains a non-finite bound",
            Self::NonPositiveRadius => "spatial cell radii must be positive",
            Self::UnsupportedScale => {
                "spatial cell requires a scale outside the established power table"
            }
            Self::CoordinateRange => {
                "spatial cell coordinate exceeds the established integer domain"
            }
        })
    }
}
impl std::error::Error for SpatialCellError {}

impl SpatialCell {
    pub fn fit(
        center: [f32; 3],
        minimum_radius: f64,
        maximum_radius: f64,
    ) -> Result<FittedSpatialCell, SpatialCellError> {
        if center.iter().any(|value| !value.is_finite())
            || !minimum_radius.is_finite()
            || !maximum_radius.is_finite()
        {
            return Err(SpatialCellError::NonFinite);
        }
        if minimum_radius <= 0.0 || maximum_radius <= 0.0 {
            return Err(SpatialCellError::NonPositiveRadius);
        }
        let diameter = minimum_radius + minimum_radius;
        if !diameter.is_finite() {
            return Err(SpatialCellError::UnsupportedScale);
        }
        let mut level = ((((diameter.to_bits() >> 52) & 0x7ff) as i32) - 1022).max(-40);
        let base = loop {
            if let Some(cell) = fit_level(center, minimum_radius, level)? {
                break cell;
            }
            level += 1;
        };
        if minimum_radius < maximum_radius
            && let Some(cell) = fit_level(center, maximum_radius, level + 1)?
        {
            return Ok(FittedSpatialCell {
                cell,
                radius: maximum_radius,
            });
        }
        Ok(FittedSpatialCell {
            cell: base,
            radius: minimum_radius,
        })
    }
}

fn fit_level(
    center: [f32; 3],
    radius: f64,
    level: i32,
) -> Result<Option<SpatialCell>, SpatialCellError> {
    let raster = level - 1;
    if !(-40..=40).contains(&raster) {
        return Err(SpatialCellError::UnsupportedScale);
    }
    let inverse = f64::from_bits(((1023 - raster) as u64) << 52);
    let mut origin = [0; 3];
    for axis in 0..3 {
        let minimum = ((f64::from(center[axis]) - radius) * inverse) as f32;
        let maximum = ((f64::from(center[axis]) + radius) * inverse) as f32;
        let minimum = minimum.floor();
        let maximum = maximum.ceil();
        if !(-2147483648.0_f32..2147483648.0_f32).contains(&minimum)
            || !(-2147483648.0_f32..2147483648.0_f32).contains(&maximum)
        {
            return Err(SpatialCellError::CoordinateRange);
        }
        let minimum = minimum as i32;
        let maximum = maximum as i32;
        if maximum
            > minimum
                .checked_add(2)
                .ok_or(SpatialCellError::CoordinateRange)?
        {
            return Ok(None);
        }
        origin[axis] = minimum;
    }
    Ok(Some(SpatialCell {
        origin,
        raster_exponent: raster,
        size_exponent: level,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fitting_rounds_grid_coordinates_to_float_before_floor_and_ceiling() {
        let ordinary = SpatialCell::fit([0.0; 3], 1.0, 1.0).unwrap();
        assert_eq!(
            ordinary.cell,
            SpatialCell {
                origin: [-1; 3],
                raster_exponent: 1,
                size_exponent: 2
            }
        );
        let adjacent = SpatialCell::fit(
            [0.0; 3],
            f64::from_bits(1.0_f64.to_bits() - 1),
            f64::from_bits(1.0_f64.to_bits() - 1),
        )
        .unwrap();
        assert_eq!(
            adjacent.cell,
            SpatialCell {
                origin: [-1; 3],
                raster_exponent: 0,
                size_exponent: 1
            }
        );
    }

    #[test]
    fn optional_expansion_tests_only_one_larger_level() {
        let fitted = SpatialCell::fit([0.0; 3], 1.0, 1.5).unwrap();
        assert_eq!(fitted.radius, 1.5);
        assert_eq!(fitted.cell.raster_exponent, 2);
        let refused = SpatialCell::fit([0.0; 3], 1.0, 10.0).unwrap();
        assert_eq!(refused.radius, 1.0);
        assert_eq!(refused.cell.raster_exponent, 1);
        assert_eq!(
            SpatialCell::fit([f32::NAN; 3], 1.0, 1.0),
            Err(SpatialCellError::NonFinite)
        );
        assert_eq!(
            SpatialCell::fit([0.0; 3], 0.0, 1.0),
            Err(SpatialCellError::NonPositiveRadius)
        );
        assert_eq!(
            SpatialCell::fit([0.0; 3], 1.0e-30, 1.0e-30),
            Err(SpatialCellError::UnsupportedScale)
        );
    }
}
