use crate::{definition::Function, dmx::Value};

pub(super) fn parameter_type(name: &str, value: &Value) -> Option<bool> {
    let suffix = name.strip_prefix("light ").and_then(|name| {
        let (number, suffix) = name.split_once(' ')?;
        matches!(number, "1" | "2" | "3" | "4").then_some(suffix)
    });
    if suffix.is_none() && !matches!(name, "initial color bias" | "clamp minimum light value to initial color" | "clamp maximum light value to initial color" | "compute normals from control points" | "half-lambert normals") { return None; }
    Some(match suffix.unwrap_or(name) {
        "control point" => matches!(value, Value::Int(_)),
        "control point offset" | "direction" => matches!(value, Value::Vector3(_)),
        "color" => matches!(value, Value::Color(_)),
        "type 0=point 1=spot" | "dynamic light" | "clamp minimum light value to initial color"
        | "clamp maximum light value to initial color" | "compute normals from control points" | "half-lambert normals" => matches!(value, Value::Bool(_)),
        "50% distance" | "0% distance" | "spot inner cone" | "spot outer cone" | "initial color bias" => matches!(value, Value::Float(_)),
        _ => return None,
    })
}

fn number(function: &Function, name: &str, default: f32) -> f32 {
    match function.parameter(name) { Some(Value::Float(value)) => *value, _ => default }
}
fn flag(function: &Function, name: &str) -> bool {
    matches!(function.parameter(name), Some(Value::Bool(true)))
}

pub(super) fn supported(function: &Function) -> bool {
    !flag(function, "Compute Normals From Control Points") && (1..=4).all(|index| {
        !flag(function, &format!("Light {index} Dynamic Light"))
            && !flag(function, &format!("Light {index} Type 0=Point 1=Spot"))
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PointLight { control: i32, offset: [f32; 3], position: [f32; 3], color: [f32; 3], attenuation: [f32; 3] }

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ControlPointLighting {
    lights: [PointLight; 4], bias: f32, clamp_minimum: bool, clamp_maximum: bool,
}

impl ControlPointLighting {
    pub(super) fn new(function: &Function, mut control: impl FnMut(i32) -> [f32; 3]) -> Self {
        let lights = std::array::from_fn(|index| {
            let location_prefix = format!("Light {}", index + 1);
            let descriptor_prefix = format!("Light {}", if index == 3 { 3 } else { index + 1 });
            let cp = match function.parameter(&format!("{location_prefix} Control Point")) { Some(Value::Int(value)) => *value, _ => 0 };
            let origin = control(cp);
            let offset = match function.parameter(&format!("{location_prefix} Control Point Offset")) { Some(Value::Vector3(value)) => *value, _ => [0.0; 3] };
            let color = match function.parameter(&format!("{descriptor_prefix} Color")) { Some(Value::Color(value)) => *value, _ => [0, 0, 0, 255] };
            PointLight { control: cp, offset, position: std::array::from_fn(|axis| origin[axis] + offset[axis]),
                color: std::array::from_fn(|axis| color[axis] as f32 / 255.0),
                attenuation: attenuation(number(function, &format!("{descriptor_prefix} 50% Distance"), 100.0), number(function, &format!("{descriptor_prefix} 0% Distance"), 200.0)),
            }
        });
        Self { lights, bias: number(function, "Initial Color Bias", 0.0),
            clamp_minimum: flag(function, "Clamp Minimum Light Value to Initial Color"),
            clamp_maximum: flag(function, "Clamp Maximum Light Value to Initial Color") }
    }

    pub(super) fn update_controls(&mut self, mut control: impl FnMut(i32) -> [f32; 3]) {
        for light in &mut self.lights {
            let origin = control(light.control);
            light.position = std::array::from_fn(|axis| origin[axis] + light.offset[axis]);
        }
    }

    pub(super) fn color(&self, position: [f32; 3], input: [f32; 3]) -> [f32; 3] {
        let mut color = input.map(|channel| channel * self.bias);
        for light in &self.lights {
            let delta: [f32; 3] = std::array::from_fn(|axis| light.position[axis] - position[axis]);
            let distance_squared = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).max(1.0);
            let [a, b, c] = light.attenuation;
            let mut denominator = if c != 0.0 { c } else { f32::EPSILON };
            if b != 0.0 { denominator += b * distance_squared.sqrt(); }
            if a != 0.0 { denominator += a * distance_squared; }
            for axis in 0..3 { color[axis] += light.color[axis] / denominator; }
        }
        std::array::from_fn(|axis| color[axis].max(if self.clamp_minimum { input[axis] } else { 0.0 })
            .min(if self.clamp_maximum { input[axis] } else { 1.0 }))
    }
}

// SDK LightDesc's two-distance attenuation, including the monotonic quadratic
// adjustment and normalization at the authored half-intensity distance.
fn attenuation(half: f32, zero: f32) -> [f32; 3] {
    let zero = if zero < half { 2.0 * half } else { zero };
    let mut points = [(0.0, 1.0), (half, 2.0), (zero, 256.0)];
    if points[0].0 > points[1].0 { points.swap(0, 1); }
    if points[1].0 > points[2].0 { points.swap(1, 2); }
    if points[0].0 > points[1].0 { points.swap(0, 1); }
    let [(x1, y1), (x2, y2), (x3, y3)] = points;
    let mut coefficients = [0.0, 1.0, 0.0];
    let determinant = (x1 - x2) * (x1 - x3) * (x2 - x3);
    let mut blend = 0.0;
    while determinant != 0.0 && blend <= 1.0 {
        let middle = (1.0 - blend) * y2 + blend * (y1 + (y3 - y1) * (x2 - x1) / (x3 - x1));
        let a = (x3 * (-y1 + middle) + x2 * (y1 - y3) + x1 * (-middle + y3)) / determinant;
        let b = (x3 * x3 * (y1 - middle) + x1 * x1 * (middle - y3) + x2 * x2 * (-y1 + y3)) / determinant;
        let c = (x1 * x3 * (-x1 + x3) * middle + x2 * x2 * (x3 * y1 - x1 * y3) + x2 * (-(x3 * x3 * y1) + x1 * x1 * y3)) / determinant;
        coefficients = [a, b, c];
        let derivative = 2.0 * a + b;
        if (y1 < y2 && y2 < y3 && derivative >= 0.0) || (y1 > y2 && y2 > y3 && derivative <= 0.0) || !(y1 < y2 && y2 < y3 || y1 > y2 && y2 > y3) { break; }
        blend += 0.05;
    }
    let [a, b, c] = coefficients;
    let scale = 2.0 / (c + half * (b + half * a));
    coefficients.map(|value| value * scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FunctionCategory;

    fn light() -> Function {
        Function { category: FunctionCategory::Operator, identity: "Color Light From Control Point".into(), element_uuid: [1; 16],
            parameters: vec![("Light 1 Color".into(), Value::Color([255, 120, 0, 255])),
                ("Light 1 50% Distance".into(), Value::Float(100.0)), ("Light 1 0% Distance".into(), Value::Float(500.0)),
                ("Initial Color Bias".into(), Value::Float(1.0))] }
    }

    #[test]
    fn half_distance_attenuation_bias_and_clamps_use_the_incoming_particle_color() {
        let mut function = light();
        let lighting = ControlPointLighting::new(&function, |_| [0.0; 3]);
        let input = [0.2, 0.3, 0.4];
        let result = lighting.color([100.0, 0.0, 0.0], input);
        assert!((result[0] - 0.7).abs() < 0.000001);
        assert!((result[1] - (0.3 + 60.0 / 255.0)).abs() < 0.000001);
        assert_eq!(result[2], 0.4);
        function.parameters.push(("Clamp Maximum Light Value to Initial Color".into(), Value::Bool(true)));
        assert_eq!(ControlPointLighting::new(&function, |_| [0.0; 3]).color([100.0, 0.0, 0.0], input), input);
    }

    #[test]
    fn rejects_unimplemented_lighting_queries_and_normals_without_ignoring_their_parameters() {
        for name in ["Compute Normals From Control Points", "Light 1 Dynamic Light", "Light 2 Type 0=Point 1=Spot"] {
            let mut function = light();
            assert!(supported(&function));
            function.parameters.push((name.into(), Value::Bool(true)));
            assert!(!supported(&function));
        }
        assert_eq!(parameter_type("light 1 color", &Value::Float(1.0)), Some(false));
        assert_eq!(parameter_type("light 5 color", &Value::Color([0; 4])), None);
        assert_eq!(parameter_type("color", &Value::Color([0; 4])), None);
    }
}
