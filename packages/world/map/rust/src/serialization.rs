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

impl HashSink {
    pub(crate) fn new() -> Self {
        Self { hash: Sha256::new(), pending: Vec::with_capacity(HASH_BUFFER_BYTES), length: 0 }
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
}

impl ByteSink for HashSink {
    #[inline(always)]
    fn extend_from_slice(&mut self, bytes: &[u8]) {
        if self.pending.len() + bytes.len() < HASH_BUFFER_BYTES {
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
    fn hash_sink_preserves_all_write_boundaries_with_bounded_storage() {
        let bytes = (0..HASH_BUFFER_BYTES * 3 + 1).map(|index| index as u8).collect::<Vec<_>>();
        for stride in [1, 2, 4, 63, 64, 65, HASH_BUFFER_BYTES - 1, HASH_BUFFER_BYTES, HASH_BUFFER_BYTES + 1, bytes.len()] {
            let mut sink = HashSink::new();
            for part in bytes.chunks(stride) {
                sink.extend_from_slice(part);
                sink.extend_from_slice(&[]);
                assert_eq!(sink.pending.capacity(), HASH_BUFFER_BYTES);
                assert!(sink.pending.len() < HASH_BUFFER_BYTES);
            }
            assert_eq!(sink.len(), bytes.len());
            assert_eq!(sink.finish(), <[u8; 32]>::from(Sha256::digest(&bytes)));
        }
        assert_eq!(HashSink::new().finish(), <[u8; 32]>::from(Sha256::digest([])));
    }
}
