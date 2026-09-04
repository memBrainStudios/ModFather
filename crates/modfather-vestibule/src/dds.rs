//! Minimal DDS header parser: just enough to extract the four fields
//! [`modfather_ba2::TextureToPack`] needs (`height`, `width`, `num_mips`,
//! `format`) and to strip the header off so only raw mip bytes get
//! packed into a DX10 BA2 chunk -- matching what Archive2 actually packs
//! (confirmed via oracle cross-validation, see
//! `modfather-ba2/tests/oracle_cross_validation.rs::our_dx10_writer_is_readable_by_independent_oracle`'s
//! doc comment: a real DX10 archive's chunk payload is the bare mip
//! bytes, with no DDS header re-attached until something explicitly
//! reconstructs a `.dds` file from them).
//!
//! **Deliberately not a full DDS/DXGI implementation:** this does not
//! decode pixels, validate block-compression layouts, or handle every
//! legacy FourCC -- that is real texture-format work
//! (`docs/CRUCIBLE.md`'s dedicated "DDS view/convert/mip job") which is
//! out of scope for what this module needs, which is solely "enough
//! metadata to fill in an `F4TexInfo` record correctly." Unrecognized or
//! unusual DDS variants (volume textures, arrays, packed/YUV formats,
//! etc.) are rejected with [`Error::UnsupportedDds`] rather than guessed
//! at silently.

use crate::packing::LooseFile;

/// Errors from [`parse`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a DDS file (bad magic or truncated header): {0}")]
    NotDds(String),
    #[error("unsupported DDS variant: {0}")]
    UnsupportedDds(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// A parsed DDS texture: metadata plus the raw mip bytes with the DDS
/// header already stripped off (i.e. exactly what a DX10 BA2 chunk's
/// payload should contain).
#[derive(Debug, Clone)]
pub struct ParsedDds {
    pub height: u16,
    pub width: u16,
    pub num_mips: u8,
    /// `DXGI_FORMAT` value, either read directly from a `DX10`-extended
    /// header or mapped from a legacy FourCC -- see [`format_from_fourcc`].
    pub format: u8,
    pub mip_data: Vec<u8>,
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    bytes
        .get(offset..offset + 4)
        .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
}

/// Map a legacy (pre-DX10-extension) FourCC to its `DXGI_FORMAT`
/// equivalent. Covers the block-compressed formats real Bethesda
/// textures overwhelmingly use; anything else is unrecognized.
fn format_from_fourcc(fourcc: u32) -> Option<u8> {
    match fourcc {
        0x3154_5844 => Some(71), // "DXT1" -> BC1_UNORM
        0x3254_5844 => Some(74), // "DXT2" -> BC2_UNORM (premultiplied alpha; same block layout)
        0x3354_5844 => Some(74), // "DXT3" -> BC2_UNORM
        0x3454_5844 => Some(77), // "DXT4" -> BC3_UNORM (premultiplied alpha; same block layout)
        0x3554_5844 => Some(77), // "DXT5" -> BC3_UNORM
        0x5531_4342 => Some(80), // "BC4U" -> BC4_UNORM
        0x5531_4954 => Some(80), // "ATI1" -> BC4_UNORM
        0x5532_4342 => Some(83), // "BC5U" -> BC5_UNORM
        0x5532_4954 => Some(83), // "ATI2" -> BC5_UNORM
        _ => None,
    }
}

/// Parse a `.dds` file's bytes into [`ParsedDds`]. Supports both legacy
/// (FourCC-only) and `DX10`-extended headers.
pub fn parse(bytes: &[u8]) -> Result<ParsedDds> {
    if bytes.len() < 128 || &bytes[0..4] != b"DDS " {
        return Err(Error::NotDds(format!("{} bytes, bad or missing magic", bytes.len())));
    }

    let height = read_u32_le(bytes, 12).ok_or_else(|| Error::NotDds("truncated header".into()))?;
    let width = read_u32_le(bytes, 16).ok_or_else(|| Error::NotDds("truncated header".into()))?;
    let mip_map_count = read_u32_le(bytes, 28).ok_or_else(|| Error::NotDds("truncated header".into()))?;
    // dwMipMapCount == 0 is legal in the DDS spec (means "just the base
    // level"); F4TexInfo::numMips is never 0 in real archives, so treat
    // that case as a single-mip texture.
    let num_mips = if mip_map_count == 0 { 1 } else { mip_map_count };

    let fourcc = read_u32_le(bytes, 84).ok_or_else(|| Error::NotDds("truncated pixel format".into()))?;

    let (format, header_len) = if fourcc == 0x3031_5844 {
        // "DX10": FourCC bytes 'D','X','1','0' read little-endian as a
        // u32 is 0x30315844.
        if bytes.len() < 148 {
            return Err(Error::NotDds("truncated DX10 extended header".into()));
        }
        let dxgi_format = read_u32_le(bytes, 128).ok_or_else(|| Error::NotDds("truncated DX10 header".into()))?;
        (dxgi_format as u8, 148usize)
    } else {
        let format = format_from_fourcc(fourcc).ok_or_else(|| {
            Error::UnsupportedDds(format!(
                "unrecognized legacy FourCC 0x{fourcc:08x} (no DX10 extended header present)"
            ))
        })?;
        (format, 128usize)
    };

    let height: u16 = height
        .try_into()
        .map_err(|_| Error::UnsupportedDds(format!("height {height} exceeds u16::MAX")))?;
    let width: u16 = width
        .try_into()
        .map_err(|_| Error::UnsupportedDds(format!("width {width} exceeds u16::MAX")))?;
    let num_mips: u8 = num_mips
        .try_into()
        .map_err(|_| Error::UnsupportedDds(format!("mip count {num_mips} exceeds u8::MAX")))?;

    Ok(ParsedDds {
        height,
        width,
        num_mips,
        format,
        mip_data: bytes[header_len..].to_vec(),
    })
}

/// Parse a [`LooseFile`] known to be a `.dds` texture (see
/// `packing::is_texture_extension`) into a
/// [`modfather_ba2::TextureToPack`], ready for [`modfather_ba2::write_dx10`].
pub fn to_texture_to_pack(file: &LooseFile) -> Result<modfather_ba2::TextureToPack> {
    let parsed = parse(&file.data)?;
    Ok(modfather_ba2::TextureToPack {
        name: file.path.replace('/', "\\"),
        data: parsed.mip_data,
        height: parsed.height,
        width: parsed.width,
        num_mips: parsed.num_mips,
        format: parsed.format,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal legacy-FourCC (`DXT5`) DDS file with the given
    /// dimensions/mip count and trailing mip bytes.
    fn build_legacy_dds(height: u32, width: u32, mip_map_count: u32, mip_data: &[u8]) -> Vec<u8> {
        let mut buf = vec![0u8; 128];
        buf[0..4].copy_from_slice(b"DDS ");
        buf[4..8].copy_from_slice(&124u32.to_le_bytes()); // dwSize
        buf[12..16].copy_from_slice(&height.to_le_bytes());
        buf[16..20].copy_from_slice(&width.to_le_bytes());
        buf[28..32].copy_from_slice(&mip_map_count.to_le_bytes());
        buf[76..80].copy_from_slice(&32u32.to_le_bytes()); // ddspf.dwSize
        buf[84..88].copy_from_slice(b"DXT5");
        buf.extend_from_slice(mip_data);
        buf
    }

    fn build_dx10_dds(
        height: u32,
        width: u32,
        mip_map_count: u32,
        dxgi_format: u32,
        mip_data: &[u8],
    ) -> Vec<u8> {
        let mut buf = vec![0u8; 148];
        buf[0..4].copy_from_slice(b"DDS ");
        buf[4..8].copy_from_slice(&124u32.to_le_bytes());
        buf[12..16].copy_from_slice(&height.to_le_bytes());
        buf[16..20].copy_from_slice(&width.to_le_bytes());
        buf[28..32].copy_from_slice(&mip_map_count.to_le_bytes());
        buf[76..80].copy_from_slice(&32u32.to_le_bytes());
        buf[84..88].copy_from_slice(b"DX10");
        buf[128..132].copy_from_slice(&dxgi_format.to_le_bytes());
        buf.extend_from_slice(mip_data);
        buf
    }

    #[test]
    fn parses_legacy_dxt5_header() {
        let mip_data = b"mip bytes repeated for compressibility ".repeat(5);
        let dds = build_legacy_dds(512, 256, 9, &mip_data);
        let parsed = parse(&dds).unwrap();
        assert_eq!(parsed.height, 512);
        assert_eq!(parsed.width, 256);
        assert_eq!(parsed.num_mips, 9);
        assert_eq!(parsed.format, 77); // BC3_UNORM
        assert_eq!(parsed.mip_data, mip_data);
    }

    #[test]
    fn parses_dx10_extended_header() {
        let mip_data = b"bc7 mip bytes repeated for compressibility ".repeat(5);
        let dds = build_dx10_dds(1024, 1024, 11, 98, &mip_data); // BC7_UNORM
        let parsed = parse(&dds).unwrap();
        assert_eq!(parsed.height, 1024);
        assert_eq!(parsed.width, 1024);
        assert_eq!(parsed.num_mips, 11);
        assert_eq!(parsed.format, 98);
        assert_eq!(parsed.mip_data, mip_data);
    }

    #[test]
    fn zero_mip_map_count_means_one_mip() {
        let dds = build_legacy_dds(32, 32, 0, b"icon bytes");
        let parsed = parse(&dds).unwrap();
        assert_eq!(parsed.num_mips, 1);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut dds = build_legacy_dds(32, 32, 1, b"x");
        dds[0] = b'X';
        assert!(matches!(parse(&dds), Err(Error::NotDds(_))));
    }

    #[test]
    fn rejects_unrecognized_legacy_fourcc() {
        let mut dds = build_legacy_dds(32, 32, 1, b"x");
        dds[84..88].copy_from_slice(b"UNKN");
        assert!(matches!(parse(&dds), Err(Error::UnsupportedDds(_))));
    }

    #[test]
    fn to_texture_to_pack_normalizes_slashes() {
        let mip_data = b"mip bytes";
        let file = LooseFile {
            path: "textures/armor/cuirass_d.dds".to_string(),
            data: build_legacy_dds(64, 64, 1, mip_data),
        };
        let tex = to_texture_to_pack(&file).unwrap();
        assert_eq!(tex.name, "textures\\armor\\cuirass_d.dds");
        assert_eq!(tex.data, mip_data);
        assert_eq!(tex.height, 64);
        assert_eq!(tex.width, 64);
    }
}
