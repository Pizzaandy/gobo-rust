use std::arch::x86_64::*;

// https://arxiv.org/pdf/1902.08318.pdf
// idk man it works

#[repr(align(16))]
struct NibbleLUT([u8; 16]);

impl NibbleLUT {
    #[inline(always)]
    fn load(&self) -> __m128i {
        unsafe { _mm_load_si128(self.0.as_ptr() as *const __m128i) }
    }
}

static HIGH_LUT: NibbleLUT = NibbleLUT([
    0b0000_0000,
    0b0000_0000,
    0b0000_0000,
    0b0000_0010,
    0b0000_0100,
    0b0000_1001,
    0b0000_0100,
    0b0000_1000,
    0b1000_0000,
    0b1000_0000,
    0b1000_0000,
    0b1000_0000,
    0b1000_0000,
    0b1000_0000,
    0b1000_0000,
    0b1000_0000,
]);

static LOW_LUT: NibbleLUT = NibbleLUT([
    0b1000_1010,
    0b1000_1110,
    0b1000_1110,
    0b1000_1110,
    0b1000_1110,
    0b1000_1110,
    0b1000_1110,
    0b1000_1110,
    0b1000_1110,
    0b1000_1110,
    0b1000_1100,
    0b1000_0100,
    0b1000_0100,
    0b1000_0100,
    0b1000_0100,
    0b1000_0101,
]);

pub fn scan_identifier(text: &[u8]) -> usize {
    if cfg!(any(target_arch = "x86", target_arch = "x86_64")) {
        if is_x86_feature_detected!("sse2") {
            return unsafe { scan_identifier_x86(text) };
        }
    }

    scan_identifier_scalar(text, 0)
}

#[target_feature(enable = "sse2")]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn scan_identifier_x86(text: &[u8]) -> usize {
    let mut i: usize = 0;
    let size = text.len();

    let high_lut = HIGH_LUT.load();
    let low_lut = LOW_LUT.load();

    unsafe {
        while (i + 16) <= size {
            let input_ptr = text.as_ptr().offset(i as isize) as *const __m128i;
            let input = _mm_loadu_si128(input_ptr);

            // check for non-ASCII characters
            if cfg!(target_feature = "sse4.1") {
                if _mm_test_all_zeros(_mm_set1_epi8(0x80u8 as i8), input) == 0 {
                    break;
                }
            } else {
                if _mm_movemask_epi8(input) != 0 {
                    break;
                }
            }

            // low nibble lookup
            let low_mask = _mm_shuffle_epi8(low_lut, input);

            // high nibble lookup
            let input_high = _mm_and_si128(_mm_srli_epi32(input, 4), _mm_set1_epi8(0x0f));
            let high_mask = _mm_shuffle_epi8(high_lut, input_high);

            let mask = _mm_and_si128(low_mask, high_mask);

            let cmp = _mm_cmpeq_epi8(mask, _mm_setzero_si128());
            let tail_mask = _mm_movemask_epi8(cmp);

            if tail_mask != 0 {
                i += tail_mask.trailing_zeros() as usize;
                return i;
            }

            i += 16;
        }
    }

    scan_identifier_scalar(text, i)
}

fn scan_identifier_scalar(text: &[u8], start: usize) -> usize {
    let mut i = start;

    while i < text.len() {
        if !is_identifier_byte(text[i]) {
            break;
        }
        i += 1;
    }

    i
}

// todo: unicode lexing?
pub const fn is_identifier_byte(c: u8) -> bool {
    matches!(c, b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'0'..=b'9')
}

pub const fn is_identifier_start(c: u8) -> bool {
    matches!(c, b'a'..=b'z' | b'A'..=b'Z' | b'_')
}
