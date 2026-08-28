use sha2::{Digest, Sha256};

pub(crate) trait ByteSink {
    fn extend_from_slice(&mut self, bytes: &[u8]);
    fn len(&self) -> usize;
    #[inline]
    fn push(&mut self, byte: u8) { self.extend_from_slice(&[byte]); }
}

impl ByteSink for Vec<u8> {
    #[inline]
    fn extend_from_slice(&mut self, bytes: &[u8]) { Vec::extend_from_slice(self, bytes); }
    #[inline]
    fn len(&self) -> usize { Vec::len(self) }
}

impl<T: ByteSink> ByteSink for &mut T {
    #[inline]
    fn extend_from_slice(&mut self, bytes: &[u8]) { (**self).extend_from_slice(bytes); }
    #[inline]
    fn len(&self) -> usize { (**self).len() }
}

// Keep scalar writes out of the SHA implementation's hot loop. This is the
// same serializer, including every authored byte, without a map-sized buffer.
pub(crate) struct HashSink {
    hash: Sha256,
    pending: Vec<u8>,
    length: usize,
}

const HASH_BUFFER_BYTES: usize = 64 * 1024;

// Numeric planes are already contiguous initialized scalars. On little-endian
// targets (including WASM), emit their exact bits once instead of routing each
// scalar through a temporary four-byte array and another append.
macro_rules! plane_writer {
    ($name:ident, $scalar:ty) => {
        pub(crate) fn $name<const N: usize>(out: &mut impl ByteSink, values: &[[$scalar; N]]) {
            #[cfg(target_endian = "little")]
            {
                // SAFETY: arrays have no padding between these numeric scalars;
                // all bytes are initialized and the view cannot outlive values.
                let bytes = unsafe {
                    std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values))
                };
                out.extend_from_slice(bytes);
            }
            #[cfg(target_endian = "big")]
            for vector in values {
                for value in vector { out.extend_from_slice(&value.to_le_bytes()); }
            }
        }
    };
}
plane_writer!(f32_plane, f32);
plane_writer!(u16_plane, u16);
plane_writer!(u32_plane, u32);

impl HashSink {
    pub(crate) fn new() -> Result<Self, std::collections::TryReserveError> {
        let mut pending = Vec::new();
        pending.try_reserve_exact(HASH_BUFFER_BYTES)?;
        Ok(Self { hash: Sha256::new(), pending, length: 0 })
    }

    pub(crate) fn finish(mut self) -> [u8; 32] {
        self.hash.update(&self.pending);
        self.hash.finalize().into()
    }

    #[inline(never)]
    fn flush(&mut self, mut bytes: &[u8]) {
        self.length += bytes.len();
        self.length += self.pending.len();
        if !self.pending.is_empty() {
            let count = HASH_BUFFER_BYTES - self.pending.len();
            self.pending.extend_from_slice(&bytes[..count]);
            self.hash.update(&self.pending);
            self.pending.clear();
            bytes = &bytes[count..];
        }
        let direct = bytes.len() / HASH_BUFFER_BYTES * HASH_BUFFER_BYTES;
        self.hash.update(&bytes[..direct]);
        self.pending.extend_from_slice(&bytes[direct..]);
        self.length -= self.pending.len();
    }

    #[inline(never)]
    fn write_plane(&mut self, bytes: &[u8]) {
        self.length += self.pending.len() + bytes.len();
        self.hash.update(&self.pending);
        self.pending.clear();
        self.hash.update(bytes);
    }
}

impl ByteSink for HashSink {
    #[inline(always)]
    fn extend_from_slice(&mut self, bytes: &[u8]) {
        if bytes.len() >= 1024 {
            self.write_plane(bytes);
        } else if self.pending.len() + bytes.len() < HASH_BUFFER_BYTES {
            self.pending.extend_from_slice(bytes);
        } else {
            self.flush(bytes);
        }
    }
    #[inline]
    fn len(&self) -> usize { self.length + self.pending.len() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_planes_match_independent_little_endian_scalar_writes() {
        let floats = [[f32::from_bits(0x8000_0000), f32::from_bits(1), f32::from_bits(0x7fc0_1234)], [f32::MIN, f32::MAX, -1.0]];
        let mut bytes = Vec::new();
        f32_plane(&mut bytes, &floats);
        assert_eq!(bytes, floats.iter().flatten().flat_map(|value| value.to_bits().to_le_bytes()).collect::<Vec<_>>());
        let shorts = [[0, 255, 256, u16::MAX]];
        bytes.clear();
        u16_plane(&mut bytes, &shorts);
        assert_eq!(bytes, shorts.iter().flatten().flat_map(|value| value.to_le_bytes()).collect::<Vec<_>>());
        let words = [[0, u32::MAX, 0x8000_0000]];
        bytes.clear();
        u32_plane(&mut bytes, &words);
        assert_eq!(bytes, words.iter().flatten().flat_map(|value| value.to_le_bytes()).collect::<Vec<_>>());
        let before = bytes.clone();
        f32_plane::<3>(&mut bytes, &[]);
        assert_eq!(bytes, before);
    }

    #[test]
    fn hash_sink_preserves_all_write_boundaries_with_bounded_storage() {
        let bytes = (0..HASH_BUFFER_BYTES * 3 + 1).map(|index| index as u8).collect::<Vec<_>>();
        for stride in [1, 2, 4, 63, 64, 65, HASH_BUFFER_BYTES - 1, HASH_BUFFER_BYTES, HASH_BUFFER_BYTES + 1, bytes.len()] {
            let mut sink = HashSink::new().unwrap();
            for part in bytes.chunks(stride) {
                sink.extend_from_slice(part);
                sink.extend_from_slice(&[]);
                assert_eq!(sink.pending.capacity(), HASH_BUFFER_BYTES);
                assert!(sink.pending.len() < HASH_BUFFER_BYTES);
            }
            assert_eq!(sink.len(), bytes.len());
            assert_eq!(sink.finish(), <[u8; 32]>::from(Sha256::digest(&bytes)));
        }
        assert_eq!(HashSink::new().unwrap().finish(), <[u8; 32]>::from(Sha256::digest([])));
    }
}
