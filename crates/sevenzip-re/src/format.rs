//! Constants from the 7z container format (`7zFormat.txt`).

/// The 6-byte magic that opens every 7z file.
pub const SIGNATURE: [u8; 6] = [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C];

/// Length of the fixed `StartHeader` block (`NextHeaderOffset` + `NextHeaderSize` + `NextHeaderCRC`).
pub const START_HEADER_LEN: usize = 20;

/// Property (structure) IDs used inside the (decoded) header stream.
#[allow(dead_code)]
pub mod property_id {
    pub const K_END: u8 = 0x00;
    pub const K_HEADER: u8 = 0x01;
    pub const K_ARCHIVE_PROPERTIES: u8 = 0x02;
    pub const K_ADDITIONAL_STREAMS_INFO: u8 = 0x03;
    pub const K_MAIN_STREAMS_INFO: u8 = 0x04;
    pub const K_FILES_INFO: u8 = 0x05;
    pub const K_PACK_INFO: u8 = 0x06;
    pub const K_UNPACK_INFO: u8 = 0x07;
    pub const K_SUBSTREAMS_INFO: u8 = 0x08;
    pub const K_SIZE: u8 = 0x09;
    pub const K_CRC: u8 = 0x0A;
    pub const K_FOLDER: u8 = 0x0B;
    pub const K_CODERS_UNPACK_SIZE: u8 = 0x0C;
    pub const K_NUM_UNPACK_STREAM: u8 = 0x0D;
    pub const K_EMPTY_STREAM: u8 = 0x0E;
    pub const K_EMPTY_FILE: u8 = 0x0F;
    pub const K_ANTI: u8 = 0x10;
    pub const K_NAME: u8 = 0x11;
    pub const K_CTIME: u8 = 0x12;
    pub const K_ATIME: u8 = 0x13;
    pub const K_MTIME: u8 = 0x14;
    pub const K_WIN_ATTRIBUTES: u8 = 0x15;
    pub const K_COMMENT: u8 = 0x16;
    pub const K_ENCODED_HEADER: u8 = 0x17;
    pub const K_START_POS: u8 = 0x18;
    pub const K_DUMMY: u8 = 0x19;
}

/// Codec (coder method) IDs. Only Copy, LZMA and LZMA2 are implemented by
/// `sevenzip-re`; everything else (BCJ, delta filters, BZip2, RAR, ...) is
/// out of scope for this standalone package.
#[allow(dead_code)]
pub mod codec_id {
    pub const COPY: &[u8] = &[0x00];
    pub const LZMA2: &[u8] = &[0x21];
    pub const LZMA: &[u8] = &[0x03, 0x01, 0x01];
    pub const BCJ_X86: &[u8] = &[0x03, 0x03, 0x01, 0x03];
    pub const DELTA: &[u8] = &[0x03];
    pub const BZIP2: &[u8] = &[0x04, 0x02, 0x02];
    pub const DEFLATE: &[u8] = &[0x04, 0x01, 0x08];
    pub const ARM64: &[u8] = &[0x0A];
    pub const ZSTD: &[u8] = &[0x04, 0xF7, 0x11, 0x01];
    pub const LZ4: &[u8] = &[0x04, 0xF7, 0x11, 0x04];
    pub const AES256_SHA256: &[u8] = &[0x06, 0xF1, 0x07, 0x01];
}
