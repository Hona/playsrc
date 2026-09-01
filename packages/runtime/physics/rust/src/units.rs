pub(crate) const METERS_PER_INCH: f32 = 0.0254;
pub(crate) const INCHES_PER_METER: f32 = 39.37008;
pub(crate) const RADIANS_PER_DEGREE: f32 = std::f32::consts::PI / 180.0;
pub(crate) const DEGREES_PER_RADIAN: f32 = f32::from_bits(0x4265_2ee1);

pub(crate) fn internal_direction(source: [f32; 3], scale: f32) -> [f32; 3] {
    [source[0] * scale, -source[2] * scale, source[1] * scale]
}
pub(crate) fn source_direction(internal: [f32; 3], scale: f32) -> [f32; 3] {
    [
        internal[0] * scale,
        internal[2] * scale,
        -internal[1] * scale,
    ]
}
pub(crate) fn internal_position(source: [f32; 3]) -> [f64; 3] {
    internal_direction(source, METERS_PER_INCH).map(f64::from)
}
pub(crate) fn source_position(internal: [f64; 3]) -> [f32; 3] {
    [
        (internal[0] * f64::from(INCHES_PER_METER)) as f32,
        (internal[2] * f64::from(INCHES_PER_METER)) as f32,
        (-internal[1] * f64::from(INCHES_PER_METER)) as f32,
    ]
}
