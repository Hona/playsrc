use crate::audio::{SoundDefinition, SoundQueryPhase, SoundSelectionState};

const SHUFFLE_SIZE: usize = 32;
const MODULUS: i32 = 2_147_483_647;
const MULTIPLIER: i32 = 16_807;
const QUOTIENT: i32 = 127_773;
const REMAINDER: i32 = 2_836;
const TABLE_DIVISOR: i32 = 67_108_864;
const MAXIMUM_INTEGER_SAMPLE: u32 = 0x7fff_ffff;
const MAXIMUM_UNIT: f32 = 1.0 - 1.2e-7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RandomSeeds {
    pub authority: i32,
    pub predicted_presentation: i32,
}

impl RandomSeeds {
    pub const INVARIANT: Self = Self {
        authority: 0,
        predicted_presentation: 0,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RandomContext {
    Authority,
    PredictedPresentation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RandomDecision {
    SoundVolume {
        definition: SoundDefinition,
        phase: SoundQueryPhase,
    },
    SoundPitch {
        definition: SoundDefinition,
        phase: SoundQueryPhase,
    },
    SoundWave {
        definition: SoundDefinition,
        phase: SoundQueryPhase,
    },
    SoundLevel {
        definition: SoundDefinition,
        phase: SoundQueryPhase,
    },
    StickyRightVelocity,
    StickyUpVelocity,
    StickyAngularY,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RandomResult {
    FloatBits(u32),
    Integer(i32),
    RejectedIntegerCandidate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RandomDraw {
    pub context: RandomContext,
    pub decision: RandomDecision,
    pub raw: i32,
    pub result: RandomResult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UniformRandomState {
    pub current: i32,
    pub shuffled: i32,
    pub table: [i32; SHUFFLE_SIZE],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RandomError {
    UnsupportedSeed,
    InvalidState,
    InvalidIntegerRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UniformRandomStream {
    state: UniformRandomState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Tf2RandomState {
    pub authority: UniformRandomState,
    pub predicted_presentation: UniformRandomState,
    pub sound_selection: SoundSelectionState,
}

impl UniformRandomStream {
    pub fn from_seed(seed: i32) -> Result<Self, RandomError> {
        if seed == i32::MIN {
            return Err(RandomError::UnsupportedSeed);
        }
        Ok(Self {
            state: UniformRandomState {
                current: if seed < 0 { seed } else { -seed },
                shuffled: 0,
                table: [0; SHUFFLE_SIZE],
            },
        })
    }

    pub fn from_state(state: UniformRandomState) -> Result<Self, RandomError> {
        let uninitialized = state.shuffled == 0 && state.current <= 0;
        let initialized = (1..MODULUS).contains(&state.current)
            && (1..MODULUS).contains(&state.shuffled)
            && state.table.iter().all(|value| (1..MODULUS).contains(value));
        if state.current == i32::MIN || (!uninitialized && !initialized) {
            return Err(RandomError::InvalidState);
        }
        Ok(Self { state })
    }

    pub fn state(&self) -> UniformRandomState {
        self.state
    }

    pub fn random_float(&mut self, minimum: f32, maximum: f32) -> f32 {
        self.random_float_observed(minimum, maximum).1
    }

    pub fn random_int(&mut self, minimum: i32, maximum: i32) -> Result<i32, RandomError> {
        self.random_int_observed(minimum, maximum, |_, _, _| {})
    }

    pub(crate) fn random_float_observed(&mut self, minimum: f32, maximum: f32) -> (i32, f32) {
        let raw = self.next_raw();
        let unit = (((raw as f64) / (MODULUS as f64)) as f32).min(MAXIMUM_UNIT);
        (raw, unit * (maximum - minimum) + minimum)
    }

    pub(crate) fn random_int_observed(
        &mut self,
        minimum: i32,
        maximum: i32,
        mut observe: impl FnMut(i32, bool, i32),
    ) -> Result<i32, RandomError> {
        let width = i64::from(maximum) - i64::from(minimum) + 1;
        if width <= 0 || width > i64::from(MAXIMUM_INTEGER_SAMPLE) + 1 {
            return Err(RandomError::InvalidIntegerRange);
        }
        if width == 1 {
            return Ok(minimum);
        }
        let width = width as u32;
        let maximum_accepted = MAXIMUM_INTEGER_SAMPLE - ((MAXIMUM_INTEGER_SAMPLE + 1) % width);
        loop {
            let raw = self.next_raw();
            let accepted = (raw as u32) <= maximum_accepted;
            let result = minimum + (raw as u32 % width) as i32;
            observe(raw, accepted, result);
            if accepted {
                return Ok(result);
            }
        }
    }

    fn next_raw(&mut self) -> i32 {
        if self.state.current <= 0 || self.state.shuffled == 0 {
            self.state.current = (-self.state.current).max(1);
            for index in (0..SHUFFLE_SIZE + 8).rev() {
                self.advance_current();
                if index < SHUFFLE_SIZE {
                    self.state.table[index] = self.state.current;
                }
            }
            self.state.shuffled = self.state.table[0];
        }

        self.advance_current();
        let index = (self.state.shuffled / TABLE_DIVISOR) as usize;
        self.state.shuffled = self.state.table[index];
        self.state.table[index] = self.state.current;
        self.state.shuffled
    }

    fn advance_current(&mut self) {
        let quotient = self.state.current / QUOTIENT;
        self.state.current =
            MULTIPLIER * (self.state.current - quotient * QUOTIENT) - REMAINDER * quotient;
        if self.state.current < 0 {
            self.state.current += MODULUS;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invariant_seed_matches_fixed_uniform_and_integer_vectors() {
        let mut stream = UniformRandomStream::from_seed(0).unwrap();
        let expected = [
            0x3ed4_fdde,
            0x3dbc_5817,
            0x3f41_a41e,
            0x3f07_9a6f,
            0x3f6e_3116,
            0x3ec4_5a62,
            0x3f27_673c,
            0x3d88_e495,
            0x3f39_0046,
            0x3f2b_d072,
            0x3ec4_4f0e,
            0x3f21_b2d0,
        ];
        for bits in expected {
            assert_eq!(stream.random_float(0.0, 1.0).to_bits(), bits);
        }

        let mut stream = UniformRandomStream::from_seed(0).unwrap();
        assert_eq!(stream.random_float(-10.0, 10.0).to_bits(), 0xbfd7_0aa8);
        assert_eq!(stream.random_float(-10.0, 10.0).to_bits(), 0xc102_923c);
        assert_eq!(stream.random_int(-1200, 1200), Ok(607));
    }

    #[test]
    fn restorable_state_and_reseed_validation_are_exact() {
        let mut stream = UniformRandomStream::from_seed(19).unwrap();
        let before = stream.state();
        let first = stream.random_float(0.0, 1.0);
        let after = stream.state();
        assert_ne!(before, after);
        assert_eq!(
            UniformRandomStream::from_state(before)
                .unwrap()
                .random_float(0.0, 1.0),
            first
        );
        assert_eq!(
            UniformRandomStream::from_state(after)
                .unwrap()
                .random_float(0.0, 1.0),
            stream.random_float(0.0, 1.0)
        );
        assert_eq!(
            UniformRandomStream::from_seed(i32::MIN),
            Err(RandomError::UnsupportedSeed)
        );
        assert_eq!(
            UniformRandomStream::from_state(UniformRandomState {
                current: 1,
                shuffled: 0,
                table: [0; SHUFFLE_SIZE],
            }),
            Err(RandomError::InvalidState)
        );
    }

    #[test]
    fn single_value_integer_does_not_advance_the_stream() {
        let mut stream = UniformRandomStream::from_seed(0).unwrap();
        let state = stream.state();
        assert_eq!(stream.random_int(4, 4), Ok(4));
        assert_eq!(stream.state(), state);
        assert_eq!(
            stream.random_int(i32::MIN, i32::MAX),
            Err(RandomError::InvalidIntegerRange)
        );
    }
}
