use crate::typed_index;
use std::arch::x86_64::*;
use std::fs;
use std::hash::{Hash, Hasher};
use std::ops::Bound;
use std::path::Path;

pub struct SourceText {
    buffer: Vec<u8>,
}

typed_index!(pub struct TextSize(u32));

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct TextSpan {
    ptr: *const u8,
    len: usize,
}

impl TextSpan {
    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl Hash for TextSpan {
    fn hash<H: Hasher>(&self, state: &mut H) {
        const OFFSET: u32 = 2166136261;
        const PRIME: u32 = 16777619;

        let mut hash = OFFSET;

        for &byte in self.as_slice() {
            hash ^= byte as u32;
            hash = hash.wrapping_mul(PRIME);
        }

        state.write_u32(hash);
    }
}

impl SourceText {
    pub fn from_str(s: &str) -> Self {
        let buffer = s.as_bytes().to_vec();
        Self::new(buffer)
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Self {
        let buffer = fs::read(path).unwrap();
        str::from_utf8(&buffer).expect("invalid utf-8");
        Self::new(buffer)
    }

    fn new(buffer: Vec<u8>) -> Self {
        assert!(buffer.len() < TextSize::MAX);
        Self { buffer }
    }

    pub fn len(&self) -> TextSize {
        TextSize::from(self.buffer.len())
    }

    pub unsafe fn get_byte_unchecked(&self, index: TextSize) -> u8 {
        debug_assert!(index < self.len());
        unsafe { *self.buffer.get_unchecked(usize::from(index)) }
    }

    pub fn get_byte(&self, index: TextSize) -> u8 {
        self.buffer[usize::from(index)]
    }

    pub fn get_slice(&self, range: impl std::ops::RangeBounds<TextSize>) -> &[u8] {
        let start = match range.start_bound() {
            Bound::Included(&s) => s,
            Bound::Excluded(&s) => s + 1,
            Bound::Unbounded => 0.into(),
        };
        let end = match range.end_bound() {
            Bound::Included(&e) => e + 1,
            Bound::Excluded(&e) => e,
            Bound::Unbounded => self.len(),
        };
        &self.buffer[start.into()..end.into()]
    }

    pub fn get_span(&self, start: TextSize, end: TextSize) -> TextSpan {
        let slice = &self.buffer[start.into()..end.into()];
        TextSpan {
            ptr: slice.as_ptr(),
            len: end.into(),
        }
    }

    pub fn find_next(&self, byte: u8, start: TextSize) -> Option<TextSize> {
        let slice = &self.buffer.as_slice()[start.into()..];
        match index_of(byte, slice) {
            Some(offset) => Some(start + offset),
            None => None,
        }
    }
}

fn index_of(byte: u8, haystack: &[u8]) -> Option<usize> {
    if cfg!(any(target_arch = "x86", target_arch = "x86_64")) {
        if is_x86_feature_detected!("sse2") {
            return unsafe { index_of_sse2(byte, haystack) };
        }
    }

    index_of_scalar(byte, haystack)
}

#[inline(always)]
fn index_of_scalar(byte: u8, haystack: &[u8]) -> Option<usize> {
    for (i, &b) in haystack.iter().enumerate() {
        if b == byte {
            return Some(i);
        }
    }
    None
}

#[target_feature(enable = "sse2")]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn index_of_sse2(byte: u8, haystack: &[u8]) -> Option<usize> {
    const SSE_CHUNK: usize = 16;

    let len = haystack.len();
    let ptr = haystack.as_ptr();
    let needle_vec = _mm_set1_epi8(byte as i8);

    let mut i = 0;

    while i + SSE_CHUNK <= len {
        let chunk = unsafe { _mm_loadu_si128(ptr.add(i) as *const __m128i) };
        let cmp = _mm_cmpeq_epi8(chunk, needle_vec);
        let mask = _mm_movemask_epi8(cmp);

        if mask != 0 {
            let offset = mask.trailing_zeros() as usize;
            return Some(i + offset);
        }

        i += SSE_CHUNK;
    }

    match index_of_scalar(byte, &haystack[i..]) {
        Some(offset) => Some(i + offset),
        None => None,
    }
}
