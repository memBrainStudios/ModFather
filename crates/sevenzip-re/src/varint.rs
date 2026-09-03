//! 7z variable-length `NUMBER` encoding.
//!
//! Per `7zFormat.txt`: the first byte's high bits indicate how many
//! extra little-endian bytes follow, and how many low bits of the
//! first byte are folded into the value.
//!
//! ```text
//! First_Byte  Extra_Bytes        Value
//! 0xxxxxxx                       ( xxxxxxx           )
//! 10xxxxxx    BYTE y[1]          (  xxxxxx << (8 * 1)) + y
//! 110xxxxx    BYTE y[2]          (   xxxxx << (8 * 2)) + y
//! ...
//! 1111110x    BYTE y[6]          (       x << (8 * 6)) + y
//! 11111110    BYTE y[7]          y
//! 11111111    BYTE y[8]          y
//! ```

use crate::error::{Error, Result};
use std::io::Read;

/// Read one 7z `NUMBER` (up to 64 bits) from `r`.
pub fn read_number<R: Read>(r: &mut R) -> Result<u64> {
    let mut first = [0u8; 1];
    r.read_exact(&mut first)?;
    let first_byte = first[0];

    let mut mask: u8 = 0x80;
    let mut value: u64 = 0;

    for i in 0..8 {
        if first_byte & mask == 0 {
            let high = (first_byte & (mask.wrapping_sub(1))) as u64;
            return Ok(value | (high << (8 * i)));
        }
        let mut byte = [0u8; 1];
        r.read_exact(&mut byte)?;
        value |= (byte[0] as u64) << (8 * i);
        mask >>= 1;
    }

    Ok(value)
}

/// Read a `NUMBER` and require it to fit in `usize`, for use as a count/index/size.
pub fn read_number_usize<R: Read>(r: &mut R) -> Result<usize> {
    let n = read_number(r)?;
    usize::try_from(n).map_err(|_| Error::Malformed("NUMBER exceeds usize".into()))
}

/// Write a value using the 7z `NUMBER` encoding (used by the packer).
///
/// Mirrors the reference 7-Zip encoder's `WriteNumber`: try `i` = 0..=7 extra
/// little-endian bytes, taking the first `i` for which `value` fits in
/// `i` extra bytes plus the folded high bits of the first byte; fall back to
/// the `0xFF` + 8-byte form for the full 64-bit range.
pub fn write_number<W: std::io::Write>(w: &mut W, value: u64) -> Result<()> {
    let mut first_byte: u8 = 0;
    let mut mask: u8 = 0x80;
    let mut num_extra: usize = 8;

    for i in 0..8u32 {
        if value < (1u64 << (7 * (i + 1))) {
            first_byte |= (value >> (8 * i)) as u8;
            num_extra = i as usize;
            break;
        }
        first_byte |= mask;
        mask >>= 1;
    }

    if num_extra == 8 {
        first_byte = 0xFF;
    }

    w.write_all(&[first_byte])?;
    let bytes = value.to_le_bytes();
    w.write_all(&bytes[..num_extra])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn roundtrip(v: u64) {
        let mut buf = Vec::new();
        write_number(&mut buf, v).unwrap();
        let mut c = Cursor::new(buf);
        let out = read_number(&mut c).unwrap();
        assert_eq!(out, v, "roundtrip failed for {v:#x}");
    }

    #[test]
    fn small_values() {
        for v in [0u64, 1, 42, 0x7F] {
            roundtrip(v);
        }
    }

    #[test]
    fn two_byte_boundary() {
        for v in [0x80u64, 0xFF, 0x100, 0x3FFF, 0x4000] {
            roundtrip(v);
        }
    }

    #[test]
    fn large_values() {
        for v in [
            0x1_0000u64,
            0xFFFF_FFFFu64,
            0x1_0000_0000u64,
            u64::MAX,
            u64::MAX - 1,
        ] {
            roundtrip(v);
        }
    }

    #[test]
    fn known_encoding_single_byte() {
        // 0x00 -> [0x00]
        let mut buf = Vec::new();
        write_number(&mut buf, 0).unwrap();
        assert_eq!(buf, vec![0x00]);
    }
}
