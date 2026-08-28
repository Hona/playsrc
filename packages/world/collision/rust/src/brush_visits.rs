use std::mem::MaybeUninit;

/// Query-local membership over the BSP's unsigned 16-bit leaf-brush indices.
/// Only touched 512-brush pages are initialized; no retained map/query state.
pub(super) struct BrushVisits {
    pages: [MaybeUninit<[u64; 8]>; 128],
    initialized: [u64; 2],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_unsigned_domain_matches_first_visit_in_any_order() {
        for stride in [1_u16, 127, 32769, 65535] {
            let mut visits = BrushVisits::new();
            for index in 0..=u16::MAX {
                let brush = index.wrapping_mul(stride);
                assert!(visits.insert(brush));
                assert!(!visits.insert(brush));
            }
            for brush in (0..=u16::MAX).rev() {
                assert!(!visits.insert(brush));
            }
        }
    }

    #[test]
    fn sparse_page_edges_and_new_queries_have_no_shared_membership() {
        for _ in 0..3 {
            let mut visits = BrushVisits::new();
            for brush in [65535, 0, 63, 64, 511, 512, 32767, 32768, 65024] {
                assert!(visits.insert(brush));
                assert!(!visits.insert(brush));
            }
        }
    }
}

impl BrushVisits {
    pub(super) fn new() -> Self {
        Self {
            pages: [MaybeUninit::uninit(); 128],
            initialized: [0; 2],
        }
    }

    pub(super) fn insert(&mut self, brush: u16) -> bool {
        let brush = usize::from(brush);
        let page = brush / 512;
        let initialized = &mut self.initialized[page / 64];
        let flag = 1_u64 << (page % 64);
        if *initialized & flag == 0 {
            self.pages[page].write([0; 8]);
            *initialized |= flag;
        }
        // The page flag is set only after writing every word. `brush` is u16,
        // so page/word indices cover exactly the fixed arrays, including 65535.
        let words = unsafe { self.pages[page].assume_init_mut() };
        let word = &mut words[(brush / 64) % 8];
        let bit = 1_u64 << (brush % 64);
        let fresh = *word & bit == 0;
        *word |= bit;
        fresh
    }
}
