use playsrc_collision::{Hull, PhysicsShape};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShapeError {
    MissingAuthoredProperties,
    MissingAuthoredConvex { convex: usize },
    Topology(crate::TopologyError),
    Orientation(crate::OrientationError),
    InvalidHull,
    MissingDragAxes,
    NonFinite,
    NonPositiveRadius,
    NonPositiveInertia { axis: usize },
}

impl fmt::Display for ShapeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingAuthoredProperties => {
                formatter.write_str("collision shape has no authored physical properties")
            }
            Self::MissingAuthoredConvex { convex } => {
                write!(
                    formatter,
                    "collision convex {convex} has no authored points"
                )
            }
            Self::MissingDragAxes => {
                formatter.write_str("collision shape has no authored drag-axis fractions")
            }
            Self::Topology(error) => error.fmt(formatter),
            Self::Orientation(error) => error.fmt(formatter),
            Self::InvalidHull => formatter.write_str("physical trace hull minimum exceeds maximum"),
            Self::NonFinite => formatter.write_str("physical shape contains a non-finite value"),
            Self::NonPositiveRadius => {
                formatter.write_str("physical shape radius must be positive")
            }
            Self::NonPositiveInertia { axis } => {
                write!(formatter, "physical inertia axis {axis} must be positive")
            }
        }
    }
}

impl std::error::Error for ShapeError {}

pub(crate) fn extreme_edge(topology: &crate::FeatureTopology, direction: [f32; 3], first: crate::EdgeId, remember: bool) -> Result<crate::EdgeId, ShapeError> {
    let score = |edge| -> Result<f32, ShapeError> {
        let point = topology.points()[topology.edge(edge).map_err(ShapeError::Topology)?.start as usize];
        let value = (point[0] * direction[0] + point[1] * direction[1]) + point[2] * direction[2];
        if !value.is_finite() { return Err(ShapeError::NonFinite); }
        Ok(value)
    };
    let mut current = first;
    let mut best = score(first)?;
    let mut seen = if remember { vec![false; topology.points().len()] } else { Vec::new() };
    if remember { seen[topology.edge(first).map_err(ShapeError::Topology)?.start as usize] = true; }
    for _ in 0..topology.edges().len() / 3 {
        let begin = topology.previous(current).map_err(ShapeError::Topology)?;
        let mut improved = None;
        for edge in topology.fan(begin).map_err(ShapeError::Topology)? {
            let vertex = topology.edge(edge).map_err(ShapeError::Topology)?.start as usize;
            if remember {
                if seen[vertex] { continue; }
                seen[vertex] = true;
            }
            let value = score(edge)?;
            if value > best { best = value; improved = Some(edge); break; }
        }
        match improved { Some(edge) => current = edge, None => break }
    }
    Ok(current)
}

pub fn source_shape_bounds(shape: &PhysicsShape, origin: [f32; 3], basis: crate::SourceAngleBasis) -> Result<Hull, ShapeError> {
    if origin.into_iter().chain(basis.matrix).any(|value| !value.is_finite()) { return Err(ShapeError::NonFinite); }
    let translated = origin != [0.0; 3];
    let offset = origin.map(|value| (value * crate::units::METERS_PER_INCH) * crate::units::INCHES_PER_METER);
    let mut result = Hull { mins: [f32::INFINITY; 3], maxs: [f32::NEG_INFINITY; 3] };
    for convex in 0..shape.convex_count() {
        let geometry = shape.authored_convex(convex).ok_or(ShapeError::MissingAuthoredConvex { convex })?;
        let first = *geometry.points.first().ok_or(ShapeError::MissingAuthoredConvex { convex })?;
        let mut indices = geometry.triangles.iter().flat_map(|triangle| triangle.edge_words()).map(|word| word & 0xffff);
        let first_index = indices.next().ok_or(ShapeError::MissingAuthoredConvex { convex })?;
        let (low, high) = indices.fold((first_index, first_index), |(low, high), index| (low.min(index), high.max(index)));
        let compact = high - low + 1 < 128 && geometry.triangles.len() <= 512;
        let linear = compact && high - low + 1 == geometry.points.len() as u32;
        let topology = if !compact || translated && !linear {
            Some(crate::FeatureTopology::from_collision(shape, convex).map_err(ShapeError::Topology)?)
        } else { None };
        for axis in 0..3 {
            let row = &basis.matrix[axis * 3..axis * 3 + 3];
            let direction = [row[0], -row[2], row[1]];
            let scaled = direction.map(|value| value * crate::units::INCHES_PER_METER);
            let coordinate = |point: [f32; 3]| {
                let a = point[0] * scaled[0] + point[1] * scaled[1];
                let b = point[2] * scaled[2];
                if translated { a + (b + offset[axis]) } else { a + b }
            };
            if !compact || translated {
                // Select support before conversion so binary32 ties remain model-space ties.
                let score = |point: [f32; 3]| (point[0] * direction[0] + point[1] * direction[1]) + point[2] * direction[2];
                let mut selected = [first; 2];
                if let Some(topology) = &topology {
                    for (index, selected) in selected.iter_mut().enumerate() {
                        let direction = direction.map(|value| if index == 0 { -value } else { value });
                        let mut seed = topology.edge_id(0).ok_or(ShapeError::MissingAuthoredConvex { convex })?;
                        if !compact {
                            seed = extreme_edge(topology, direction.map(|value| if value < 0.0 { -1.0 } else { 1.0 }), seed, false)?;
                        }
                        let edge = extreme_edge(topology, direction, seed, true)?;
                        *selected = topology.points()[topology.edge(edge).map_err(ShapeError::Topology)?.start as usize];
                    }
                } else {
                    let mut scores = [score(first); 2];
                    for &point in &geometry.points[1..] {
                        let value = score(point);
                        if value < scores[0] { selected[0] = point; scores[0] = value; }
                        if value > scores[1] { selected[1] = point; scores[1] = value; }
                    }
                }
                result.mins[axis] = result.mins[axis].min(coordinate(selected[0]));
                result.maxs[axis] = result.maxs[axis].max(coordinate(selected[1]));
            } else {
                for &point in &geometry.points {
                    let value = coordinate(point);
                    result.mins[axis] = result.mins[axis].min(value);
                    result.maxs[axis] = result.maxs[axis].max(value);
                }
            }
        }
    }
    if result.mins.into_iter().chain(result.maxs).any(|value| !value.is_finite()) { return Err(ShapeError::NonFinite); }
    Ok(result)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicalShape {
    drag_mass_frame: Option<([f32; 3], f32)>,
    pub mass: f32,
    pub inertia_factor: f32,
    pub rotational_inertia_limit: f32,
    pub restored_inertia: Option<[f32; 3]>,
    pub center: [f32; 3],
    pub inertia: [f32; 3],
    pub authored_radius: f32,
    pub radius: f32,
    pub inverse_diameter: f32,
    pub max_surface_deviation: u8,
    pub surface_deviation_radius: f32,
    pub linear_drag_basis: [f32; 3],
    pub angular_drag_basis: [f32; 3],
}

impl PhysicalShape {
    pub fn uses_simple_rotation(&self) -> bool {
        let inverse = self.inertia.map(|value| 1.0 / value);
        let a = inverse[1] - inverse[2];
        let b = inverse[2] - inverse[0];
        let c = inverse[0] - inverse[1];
        let difference = (a * a + b * b) + c * c;
        let magnitude =
            (inverse[0] * inverse[0] + inverse[1] * inverse[1]) + inverse[2] * inverse[2];
        f64::from(magnitude) * 0.010_000_000_298_023_226 > f64::from(difference)
    }
    pub fn from_collision(
        shape: &PhysicsShape,
        mass: f32,
        inertia_factor: f32,
        rotational_inertia_limit: f32,
    ) -> Result<Self, ShapeError> {
        if !mass.is_finite() || !inertia_factor.is_finite() || !rotational_inertia_limit.is_finite()
        {
            return Err(ShapeError::NonFinite);
        }
        let mass = mass.clamp(0.1, 50_000.0);
        let inertia_factor = if inertia_factor <= 0.0 {
            1.0
        } else {
            inertia_factor.min(1.0e18)
        };
        Self::derive(
            shape,
            mass,
            inertia_factor,
            rotational_inertia_limit,
            None,
            None,
        )
    }
    pub(crate) fn from_archive(
        shape: &PhysicsShape,
        mass: f32,
        inertia: [f32; 3],
    ) -> Result<Self, ShapeError> {
        if !mass.is_finite() || mass <= 0.0 || inertia.iter().any(|v| !v.is_finite() || *v <= 0.0) {
            return Err(ShapeError::NonFinite);
        }
        Self::derive(shape, mass, 1.0, 0.03, Some(inertia), None)
    }
    pub(crate) fn with_core_mass_frame(
        &self,
        shape: &PhysicsShape,
        mass: f32,
        inertia: [f32; 3],
    ) -> Result<Self, ShapeError> {
        Self::derive(
            shape,
            mass,
            self.inertia_factor,
            self.rotational_inertia_limit,
            Some(inertia),
            Some(self.drag_mass_frame.unwrap_or((self.inertia, self.mass))),
        )
    }
    pub(crate) fn static_drag_bases(mut self) -> Self {
        self.linear_drag_basis = [0.0; 3];
        self.angular_drag_basis = [0.0; 3];
        self
    }
    pub(crate) fn validate(
        &self,
        shape: &PhysicsShape,
        is_static: bool,
    ) -> Result<bool, ShapeError> {
        let mut expected = if let Some(inertia) = self.restored_inertia {
            if !self.mass.is_finite()
                || self.mass <= 0.0
                || inertia.iter().any(|v| !v.is_finite() || *v <= 0.0)
            {
                return Err(ShapeError::NonFinite);
            }
            Self::derive(
                shape,
                self.mass,
                self.inertia_factor,
                self.rotational_inertia_limit,
                Some(inertia),
                self.drag_mass_frame,
            )?
        } else {
            if self.drag_mass_frame.is_some() {
                return Ok(false);
            }
            Self::from_collision(
                shape,
                self.mass,
                self.inertia_factor,
                self.rotational_inertia_limit,
            )?
        };
        if is_static {
            expected = expected.static_drag_bases();
        }
        Ok(*self == expected)
    }
    fn derive(
        shape: &PhysicsShape,
        mass: f32,
        inertia_factor: f32,
        rotational_inertia_limit: f32,
        restored_inertia: Option<[f32; 3]>,
        drag_mass_frame: Option<([f32; 3], f32)>,
    ) -> Result<Self, ShapeError> {
        let source = shape
            .authored_properties()
            .ok_or(ShapeError::MissingAuthoredProperties)?;
        let drag = source.drag_axes.ok_or(ShapeError::MissingDragAxes)?;
        if source
            .center
            .iter()
            .chain(source.inertia.iter())
            .chain(drag.iter())
            .any(|component| !component.is_finite())
            || !source.radius.is_finite()
        {
            return Err(ShapeError::NonFinite);
        }
        for (axis, inertia) in source.inertia.iter().enumerate() {
            if *inertia <= 0.0 {
                return Err(ShapeError::NonPositiveInertia { axis });
            }
        }
        let mut minimum = [f32::INFINITY; 3];
        let mut maximum = [f32::NEG_INFINITY; 3];
        let radius = source.radius;
        for convex in 0..shape.convex_count() {
            let convex = shape
                .authored_convex(convex)
                .ok_or(ShapeError::MissingAuthoredConvex { convex })?;
            for point in &convex.points {
                for axis in 0..3 {
                    minimum[axis] = minimum[axis].min(point[axis]);
                    maximum[axis] = maximum[axis].max(point[axis]);
                }
            }
        }
        let extent: [f32; 3] = std::array::from_fn(|axis| {
            let source_maximum = maximum[axis] * f32::from_bits(0x421d_7af5);
            let source_minimum = minimum[axis] * f32::from_bits(0x421d_7af5);
            (source_maximum - source_minimum) * f32::from_bits(0x3cd0_13a9)
        });
        if drag_mass_frame.is_some_and(|(inertia, mass)| {
            !mass.is_finite() || mass <= 0.0 || inertia.iter().any(|v| !v.is_finite() || *v <= 0.0)
        }) {
            return Err(ShapeError::NonFinite);
        }
        let inverse_mass = 1.0 / drag_mass_frame.map_or(mass, |frame| frame.1);
        if radius <= 0.0 {
            return Err(ShapeError::NonPositiveRadius);
        }
        let inverse_diameter = 0.5_f32 / radius;
        let linear_drag_basis = [
            extent[1] * extent[2] * drag[0] * inverse_mass,
            extent[0] * extent[2] * drag[1] * inverse_mass,
            extent[0] * extent[1] * drag[2] * inverse_mass,
        ];
        let mut inertia = source
            .inertia
            .map(|component| (f64::from(component * inertia_factor) * f64::from(mass)) as f32);
        if rotational_inertia_limit != 0.0 {
            let squared = inertia.map(|value| f64::from(value) * f64::from(value));
            let minimum = (((squared[0] + squared[1]) + squared[2]).sqrt()
                * f64::from(rotational_inertia_limit)) as f32;
            for axis in &mut inertia {
                if *axis < minimum {
                    *axis = minimum;
                }
            }
        }
        for axis in &mut inertia {
            if !axis.is_finite() || *axis > 1.0e18_f32 {
                *axis = 1.0e18_f32;
            } else if *axis < -1.0e18_f32 {
                *axis = -1.0e18_f32;
            }
        }
        if let Some(restored) = restored_inertia {
            inertia = restored;
        }
        let surface_deviation_radius =
            (f32::from(source.max_surface_deviation) * f32::from_bits(0x3b83_126f)) * radius;
        let inverse_inertia = drag_mass_frame
            .map_or(inertia, |frame| frame.0)
            .map(|component| 1.0 / component);
        let half = extent.map(|component| component * 0.5);
        let angular_drag_basis = [
            drag[2] * angular_integral(inverse_inertia[0], half[0], half[1], half[2])
                + drag[1] * angular_integral(inverse_inertia[0], half[0], half[2], half[1]),
            drag[2] * angular_integral(inverse_inertia[1], half[1], half[0], half[2])
                + drag[0] * angular_integral(inverse_inertia[1], half[1], half[2], half[0]),
            drag[1] * angular_integral(inverse_inertia[2], half[2], half[0], half[1])
                + drag[0] * angular_integral(inverse_inertia[2], half[2], half[1], half[0]),
        ];
        if !radius.is_finite()
            || !inverse_diameter.is_finite()
            || !surface_deviation_radius.is_finite()
            || inertia
                .iter()
                .chain(linear_drag_basis.iter())
                .chain(angular_drag_basis.iter())
                .any(|component| !component.is_finite())
        {
            return Err(ShapeError::NonFinite);
        }
        Ok(Self {
            drag_mass_frame,
            mass,
            inertia_factor,
            rotational_inertia_limit,
            restored_inertia,
            center: source.center,
            inertia,
            authored_radius: source.radius,
            radius,
            inverse_diameter,
            max_surface_deviation: source.max_surface_deviation,
            surface_deviation_radius,
            linear_drag_basis,
            angular_drag_basis,
        })
    }
}

fn angular_integral(inverse_inertia: f32, length: f32, width: f32, height: f32) -> f32 {
    let width_squared = width * width;
    let length_squared = length * length;
    let height_squared = height * height;
    inverse_inertia
        * ((1.0 / 3.0) * width_squared * length * length_squared
            + 0.5 * width_squared * width_squared * length
            + length * width_squared * height_squared)
}

#[cfg(test)]
mod tests {
    use super::{PhysicalShape, ShapeError, source_shape_bounds};
    use playsrc_collision::{ConvexInput, PhysicsShape, SnapshotLimits};

    #[test]
    fn generic_collision_geometry_never_fabricates_physical_shape_properties() {
        let generic = PhysicsShape::compile(
            1,
            vec![ConvexInput {
                solid: 0,
                convex: 0,
                contents: 1,
                vertices: vec![
                    [0.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                    [0.0, 0.0, 1.0],
                ],
                triangles: vec![[0, 1, 2], [0, 3, 1], [0, 2, 3], [1, 3, 2]],
                authored: None,
            }],
            SnapshotLimits::default(),
        )
        .unwrap();
        let basis = crate::SourceAngleBasis::from_degrees([0.0; 3]).unwrap();
        assert_eq!(source_shape_bounds(&generic, [0.0; 3], basis), Err(ShapeError::MissingAuthoredConvex { convex: 0 }));
        assert_eq!(source_shape_bounds(&generic, [f32::NAN; 3], basis), Err(ShapeError::NonFinite));
        assert_eq!(source_shape_bounds(&generic, [0.0; 3], crate::SourceAngleBasis { matrix: [f32::INFINITY; 9] }), Err(ShapeError::NonFinite));
        assert_eq!(
            PhysicalShape::from_collision(&generic, 5.0, 1.0, 0.05),
            Err(ShapeError::MissingAuthoredProperties)
        );
        assert_eq!(
            PhysicalShape::from_collision(&generic, 0.0, 1.0, 0.05),
            Err(ShapeError::MissingAuthoredProperties)
        );
        assert_eq!(
            PhysicalShape::from_collision(&generic, f32::NAN, 1.0, 0.05),
            Err(ShapeError::NonFinite)
        );
    }
}
