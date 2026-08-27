//! Automatic room classification and construction from the configured presets.
use crate::dsp::{Error, Preset, Presets};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Room {
    pub outside: bool,
    pub width: i32,
    pub length: i32,
    pub height: i32,
    pub diffusion: f32,
    pub reflectivity: f32,
    pub surfaces: [f32; 6],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Shape {
    Room,
    Duct,
    Hall,
    Tunnel,
    Street,
    Alley,
    Courtyard,
    OpenSpace,
    Wall,
    OpenStreet,
    OpenCourtyard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Classification {
    pub shape: Shape,
    pub size: usize,
    pub width: usize,
    pub length: usize,
    pub height: usize,
    pub reflectivity: usize,
    pub diffusion: usize,
}

fn dimension(value: i32, edges: [i32; 4]) -> usize {
    edges.iter().filter(|edge| value > **edge).count()
}

impl Room {
    pub fn classify(self) -> Classification {
        let (length, height) = if !self.outside && self.height as f64 > 3.0 * self.length as f64 {
            (self.height, self.length)
        } else {
            (self.length, self.height)
        };
        let width = self.width;
        let shape = if !self.outside {
            if length as f64 > 4.0 * width as f64 && width <= 48 {
                Shape::Duct
            } else if length as f64 > 2.5 * width as f64 && width > 48 && width <= 96 {
                Shape::Hall
            } else if length as f64 > 4.0 * width as f64 && width > 96 {
                Shape::Tunnel
            } else {
                Shape::Room
            }
        } else {
            match self.surfaces[..4]
                .iter()
                .filter(|value| **value == 0.0)
                .count()
            {
                4 => Shape::OpenSpace,
                3 => Shape::Wall,
                2 => Shape::OpenStreet,
                1 => Shape::OpenCourtyard,
                _ if length as f64 <= 2.5 * width as f64 => Shape::Courtyard,
                _ if width <= 144 => Shape::Alley,
                _ => Shape::Street,
            }
        };
        let length_class = dimension(length, [144, 288, 576, 1152]);
        let width_class = dimension(width, [72, 144, 288, 576]);
        let size = match shape {
            Shape::Courtyard | Shape::OpenCourtyard => dimension(length, [120, 240, 480, 1200]),
            Shape::Street | Shape::Alley => width_class,
            _ => (length_class + width_class) / 2,
        };
        Classification {
            shape,
            size,
            width: width_class,
            length: length_class,
            height: dimension(height, [48, 128, 216, 384]),
            reflectivity: [0.04_f64, 0.50, 0.80]
                .iter()
                .filter(|edge| f64::from(self.reflectivity) > **edge)
                .count(),
            diffusion: [0.01_f64, 0.1, 0.3]
                .iter()
                .filter(|edge| f64::from(self.diffusion) > **edge)
                .count(),
        }
    }
}

/// First template of each shape, in the order of `Shape`. Templates are loaded
/// from content; this table is the configured automatic-DSP convar default.
pub const DEFAULT_TEMPLATES: [usize; 11] = [102, 106, 110, 114, 118, 122, 126, 130, 130, 118, 126];

impl Presets {
    pub fn automatic(
        &self,
        room: Room,
        templates: &[usize; 11],
    ) -> Result<(Classification, Preset), Error> {
        if room.width < 0
            || room.length < 0
            || room.height < 0
            || !room.diffusion.is_finite()
            || !room.reflectivity.is_finite()
            || room.surfaces.iter().any(|value| !value.is_finite())
        {
            return Err(Error::Malformed("room measurement"));
        }
        let class = room.classify();
        let index = templates[class.shape as usize] + if class.diffusion > 1 { 2 } else { 0 };
        let low = self
            .0
            .get(index)
            .ok_or(Error::Malformed("missing automatic minimum template"))?;
        let high = self
            .0
            .get(index + 1)
            .ok_or(Error::Malformed("missing automatic maximum template"))?;
        let mut result = if class.size > 1 {
            high.clone()
        } else {
            low.clone()
        };
        for skip in 0..2 {
            // Match occurrences, not indexes: small and large templates may
            // contain different processors.
            for kind in 1..=11 {
                let Some(target) = result
                    .processors
                    .iter_mut()
                    .filter(|entry| entry.kind == kind)
                    .nth(skip)
                else {
                    continue;
                };
                let Some(low) = low
                    .processors
                    .iter()
                    .filter(|entry| entry.kind == kind)
                    .nth(skip)
                else {
                    continue;
                };
                let Some(high) = high
                    .processors
                    .iter()
                    .filter(|entry| entry.kind == kind)
                    .nth(skip)
                else {
                    continue;
                };
                for parameter in 0..16 {
                    let rule = match kind {
                        10 if parameter < 4 => Some((class.size, 5, false, false, None)),
                        2 => match parameter {
                            0 | 1 => Some((class.size, 5, true, false, None)),
                            2 | 3 | 4 | 7 | 8 => Some((class.size, 5, false, false, None)),
                            5 => Some((class.reflectivity, 4, false, true, None)),
                            9..=11 => {
                                let axis = parameter - 9;
                                let bins = [class.width, class.length, class.height];
                                let dimensions = [room.width, room.length, room.height];
                                Some((
                                    bins[axis],
                                    5,
                                    true,
                                    false,
                                    Some((dimensions[axis] as f32 as f64 / 12.0).clamp(6.0, 500.0)
                                        as f32),
                                ))
                            }
                            12..=14 => Some((
                                [class.width, class.length, class.height][parameter - 12],
                                5,
                                false,
                                false,
                                None,
                            )),
                            _ => None,
                        },
                        1 => match parameter {
                            1 | 8..=10 => Some((
                                class.length,
                                5,
                                true,
                                false,
                                Some(
                                    (f64::from(if parameter == 8 {
                                        room.width
                                    } else {
                                        room.length
                                    }) * 2.0
                                        / 12.0)
                                        .clamp(14.0, 500.0)
                                        as f32,
                                ),
                            )),
                            2 | 3 => Some((class.length, 5, false, false, None)),
                            5 | 6 => Some((class.length, 5, false, true, None)),
                            _ => None,
                        },
                        9 if parameter < 13 => Some((class.length, 5, false, false, None)),
                        3 if parameter < 5 => Some((class.size, 5, false, false, None)),
                        // Other processor families are not silently retyped.
                        4..=8 | 11 => Some((class.size, 5, false, false, None)),
                        _ => None,
                    };
                    let Some((bin, bins, squared, reverse, direct)) = rule else {
                        continue;
                    };
                    let (a, b) = if reverse {
                        (high.parameters[parameter], low.parameters[parameter])
                    } else {
                        (low.parameters[parameter], high.parameters[parameter])
                    };
                    target.parameters[parameter] =
                        if let Some(value) = direct.filter(|_| a < 0.0 || b < 0.0) {
                            value
                        } else {
                            let fraction = bin as f32 / bins as f32;
                            if squared {
                                a + (b - a) * fraction * fraction
                            } else {
                                a + (b - a) * fraction
                            }
                        };
                }
            }
        }
        Ok((class, result))
    }
}
