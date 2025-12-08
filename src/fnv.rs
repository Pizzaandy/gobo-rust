use std::hash::Hasher;

const FNV_32_OFFSET: u32 = 0x811c9dc5;
const FNV_32_PRIME: u32 = 0x01000193;

#[inline]
pub fn fnv1a_32(data: &[u8]) -> u32 {
    let mut hasher = Fnv1aHasher32::new();
    hasher.write(data);
    hasher.finish_raw()
}

#[derive(Debug, Copy, Clone)]
pub struct Fnv1aHasher32 {
    state: u32,
}

impl Fnv1aHasher32 {
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            state: FNV_32_OFFSET,
        }
    }

    #[inline(always)]
    pub fn finish_raw(&self) -> u32 {
        self.state
    }
}

impl Default for Fnv1aHasher32 {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

impl std::hash::Hasher for Fnv1aHasher32 {
    #[inline(always)]
    fn finish(&self) -> u64 {
        self.state as u64
    }

    #[inline(never)]
    fn write(&mut self, bytes: &[u8]) {
        let mut state = self.state;
        for &b in bytes {
            state ^= b as u32;
            state = state.wrapping_mul(FNV_32_PRIME);
        }
        self.state = state;
    }
}
