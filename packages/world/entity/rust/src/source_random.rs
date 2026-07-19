// Adapted from Valve's Source 1 SDK uniform random-stream contract and constants.
// Copyright Valve Corporation. Subject to packages/world/entity/SOURCE-1-SDK-LICENSE.txt;
// the repository includes the SDK third-party notices under packages/presentation/particle.

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceRandom {
    seed: i32,
    shuffle_value: i32,
    shuffle: [i32; 32],
}

impl SourceRandom {
    pub(crate) fn new(seed: i32) -> Self {
        Self {
            seed: if seed < 0 { seed } else { seed.wrapping_neg() },
            shuffle_value: 0,
            shuffle: [0; 32],
        }
    }

    pub(crate) fn integer(&mut self, minimum: i32, maximum: i32) -> i32 {
        let width = i64::from(maximum) - i64::from(minimum) + 1;
        if !(2..=i64::from(i32::MAX) + 1).contains(&width) {
            return minimum;
        }
        let width = width as u32;
        let maximum_accepted = u32::MAX >> 1;
        let maximum_accepted = maximum_accepted - ((maximum_accepted + 1) % width);
        let sample = loop {
            let sample = self.next() as u32;
            if sample <= maximum_accepted {
                break sample;
            }
        };
        minimum.wrapping_add((sample % width) as i32)
    }

    pub(crate) fn float(&mut self, minimum: f32, maximum: f32) -> f32 {
        let unit = ((1.0_f64 / 2_147_483_647.0) * f64::from(self.next())).min(1.0 - 1.2e-7) as f32;
        unit * (maximum - minimum) + minimum
    }

    fn next(&mut self) -> i32 {
        const MULTIPLIER: i32 = 16_807;
        const MODULUS: i32 = 2_147_483_647;
        const QUOTIENT: i32 = 127_773;
        const REMAINDER: i32 = 2_836;
        const DIVISOR: i32 = 1 + (MODULUS - 1) / 32;

        if self.seed <= 0 || self.shuffle_value == 0 {
            self.seed = if self.seed.wrapping_neg() < 1 {
                1
            } else {
                self.seed.wrapping_neg()
            };
            for index in (0..40).rev() {
                self.advance(MULTIPLIER, MODULUS, QUOTIENT, REMAINDER);
                if index < 32 {
                    self.shuffle[index] = self.seed;
                }
            }
            self.shuffle_value = self.shuffle[0];
        }
        self.advance(MULTIPLIER, MODULUS, QUOTIENT, REMAINDER);
        let index = (self.shuffle_value / DIVISOR).clamp(0, 31) as usize;
        self.shuffle_value = self.shuffle[index];
        self.shuffle[index] = self.seed;
        self.shuffle_value
    }

    fn advance(&mut self, multiplier: i32, modulus: i32, quotient: i32, remainder: i32) {
        let division = self.seed / quotient;
        self.seed = multiplier
            .wrapping_mul(self.seed - division * quotient)
            .wrapping_sub(remainder * division);
        if self.seed < 0 {
            self.seed = self.seed.wrapping_add(modulus);
        }
    }

    pub(crate) fn encode(
        &self,
        output: &mut crate::world::SnapshotWriter,
    ) -> Result<(), super::RuntimeFailure> {
        output.i32(self.seed)?;
        output.i32(self.shuffle_value)?;
        for value in self.shuffle {
            output.i32(value)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SourceRandom;

    #[test]
    fn fixed_stream_repeats_float_and_unbiased_integer_samples() {
        let mut first = SourceRandom::new(1_337);
        let values = [
            first.float(-2.0, 4.0).to_bits(),
            first.float(-2.0, 4.0).to_bits(),
            first.float(-2.0, 4.0).to_bits(),
        ];
        let integers = [
            first.integer(0, 15),
            first.integer(0, 15),
            first.integer(0, 15),
        ];
        assert_eq!(values, [1_065_543_252, 1_081_969_206, 3_207_539_162]);
        assert_eq!(integers, [6, 3, 1]);
        let mut second = SourceRandom::new(1_337);
        assert_eq!(
            values,
            [
                second.float(-2.0, 4.0).to_bits(),
                second.float(-2.0, 4.0).to_bits(),
                second.float(-2.0, 4.0).to_bits(),
            ]
        );
        assert_eq!(
            integers,
            [
                second.integer(0, 15),
                second.integer(0, 15),
                second.integer(0, 15),
            ]
        );
    }
}
