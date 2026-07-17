use std::collections::BTreeMap;

use crate::{Error, ErrorCode};

const SAMPLE_COUNT: usize = 1_024;

#[derive(Clone, Debug, PartialEq)]
pub struct SheetFrame {
    pub duration_seconds: f32,
    pub images: [[f32; 4]; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct SheetSequence {
    pub clamp: bool,
    pub duration_seconds: f32,
    pub frames: Vec<SheetFrame>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParticleSheet {
    pub sequences: BTreeMap<i32, SheetSequence>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SheetSample {
    pub current: [[f32; 4]; 4],
    pub next: [[f32; 4]; 4],
    pub blend: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SheetSampleRequest {
    pub sequence: i32,
    pub age_seconds: f32,
    pub lifetime_seconds: f32,
    pub animation_rate: f32,
    pub fit_lifetime: bool,
    pub animation_rate_as_fps: bool,
}

pub fn sample_sheet(
    sheet: &ParticleSheet,
    request: SheetSampleRequest,
) -> Result<SheetSample, Error> {
    validate_request(request)?;
    let sequence = sheet
        .sequences
        .get(&request.sequence)
        .or_else(|| sheet.sequences.first_key_value().map(|(_, value)| value))
        .ok_or_else(|| {
            Error::new(
                ErrorCode::MissingDependency,
                "particle-sheet",
                0,
                "particle sheet contains no sequence",
            )
        })?;
    validate_sequence(sequence)?;
    if sequence.frames.len() == 1 {
        return Ok(SheetSample {
            current: sequence.frames[0].images,
            next: sequence.frames[0].images,
            blend: 0.0,
        });
    }
    let age_scale = if request.fit_lifetime {
        SAMPLE_COUNT as f32 / request.lifetime_seconds
    } else if request.animation_rate_as_fps {
        request.animation_rate * SAMPLE_COUNT as f32 / sequence.duration_seconds
    } else {
        request.animation_rate * SAMPLE_COUNT as f32
    };
    let raw = (request.age_seconds * age_scale).floor().max(0.0) as usize;
    let index = if sequence.clamp {
        raw.min(SAMPLE_COUNT - 1)
    } else {
        raw & (SAMPLE_COUNT - 1)
    };
    let mut knots = Vec::with_capacity(sequence.frames.len());
    let mut elapsed = 0.0;
    for frame in &sequence.frames {
        knots.push(SAMPLE_COUNT as f32 * elapsed / sequence.duration_seconds);
        elapsed += frame.duration_seconds;
    }
    let bracket = knots.iter().position(|position| *position >= index as f32);
    let (current, next, offset, width) = match bracket {
        Some(0) if sequence.clamp => (0, 0, 1.0, 1.0),
        Some(0) => {
            let last = sequence.frames.len() - 1;
            let width = knots[0] + SAMPLE_COUNT as f32 - knots[last];
            (
                last,
                0,
                index as f32 + SAMPLE_COUNT as f32 - knots[last],
                width,
            )
        }
        Some(next) => {
            let current = next - 1;
            (
                current,
                next,
                index as f32 - knots[current],
                knots[next] - knots[current],
            )
        }
        None if sequence.clamp => {
            let last = sequence.frames.len() - 1;
            (last, last, 1.0, 1.0)
        }
        None => {
            let last = sequence.frames.len() - 1;
            (
                last,
                0,
                index as f32 - knots[last],
                SAMPLE_COUNT as f32 - knots[last],
            )
        }
    };
    Ok(SheetSample {
        current: sequence.frames[current].images,
        next: sequence.frames[next].images,
        blend: (offset / width).clamp(0.0, 1.0),
    })
}

fn validate_request(request: SheetSampleRequest) -> Result<(), Error> {
    if ![
        request.age_seconds,
        request.lifetime_seconds,
        request.animation_rate,
    ]
    .iter()
    .all(|value| value.is_finite())
        || request.age_seconds < 0.0
        || request.lifetime_seconds <= 0.0
        || request.animation_rate < 0.0
    {
        return Err(Error::new(
            ErrorCode::InvalidValue,
            "particle-sheet",
            0,
            "sheet sample request is invalid",
        ));
    }
    Ok(())
}

fn validate_sequence(sequence: &SheetSequence) -> Result<(), Error> {
    if sequence.frames.is_empty()
        || !sequence.duration_seconds.is_finite()
        || sequence.duration_seconds <= 0.0
        || sequence.frames.len() > SAMPLE_COUNT
    {
        return Err(Error::new(
            ErrorCode::InvalidValue,
            "particle-sheet",
            0,
            "sheet sequence header is invalid",
        ));
    }
    let mut total = 0.0;
    for frame in &sequence.frames {
        if !frame.duration_seconds.is_finite()
            || frame.duration_seconds <= 0.0
            || !frame.images.iter().flatten().all(|value| value.is_finite())
        {
            return Err(Error::new(
                ErrorCode::InvalidValue,
                "particle-sheet",
                0,
                "sheet frame is invalid",
            ));
        }
        total += frame.duration_seconds;
    }
    if (total - sequence.duration_seconds).abs() > 1.0e-4 * sequence.duration_seconds.max(1.0) {
        return Err(Error::new(
            ErrorCode::InvalidValue,
            "particle-sheet",
            0,
            "sheet frame durations do not equal the sequence duration",
        ));
    }
    Ok(())
}
