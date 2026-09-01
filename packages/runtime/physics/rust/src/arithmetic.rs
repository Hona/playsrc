pub(crate) fn refined_inverse_root<const STEPS: usize>(squared: f64) -> f64 {
    let upper = (squared.to_bits() >> 32) as u32;
    let exponent = (0x7ff0_0000_u32.wrapping_sub(upper) as i32) >> 1;
    let mut inverse = f64::from_bits(u64::from((exponent as u32).wrapping_add(0x1ff0_0000)) << 32);
    let half = squared * 0.5;
    for _ in 0..STEPS {
        inverse *= (0.5 - (inverse * inverse) * half) + 1.0;
    }
    inverse
}
