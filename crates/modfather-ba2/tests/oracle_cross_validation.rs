//! Cross-validation against an independently-written BA2 implementation.
//!
//! Same rationale as `modfather-bsa/tests/oracle_cross_validation.rs`:
//! genuine game-shipped BA2 files are Bethesda's copyrighted assets and are
//! not available in this sandbox, so a same-crate round trip (see
//! `write_roundtrip.rs`, `gnrl_roundtrip.rs`) can only prove this crate is
//! *self-consistent*, not that it agrees with the real on-disk format as
//! anyone else would implement it.
//!
//! This test uses the `ba2` crate (crates.io, 0BSD license, written
//! independently of ModFather) purely as a **dev-dependency test oracle**:
//! - `ba2::fo4::Archive::write` produces a BA2 that this crate's reader
//!   must parse and decode correctly.
//! - This crate's `write` produces a BA2 that `ba2::fo4::Archive::read`
//!   must parse and decode correctly.
//!
//! `ba2` is never a runtime dependency of `modfather-ba2` or of the
//! ModFather product; it appears only in `[dev-dependencies]` and only in
//! this test file.
//!
//! This test specifically exercises the header-extension and per-archive
//! codec bugs fixed in `format.rs`/`reader.rs`/`writer.rs`: v1 (base
//! 24-byte header, always zlib), v2 (32-byte header, always zlib), and v3
//! (36-byte header, `compression_method` field selecting zlib *or* LZ4).
//! Before that fix, none of the v2/v3 cases below would have round-tripped
//! through the independent oracle.
//!
//! **Upstream bug workaround (LZ4 only):** `ba2` v3.0.1's own
//! `fo4::Chunk::decompress_into` is broken for the LZ4 branch, independent
//! of anything this crate does. `decompress_into` calls
//! `out.reserve_exact(decompressed_len)` on a fresh `Vec<u8>` -- which
//! only grows *capacity*, not *length* -- then passes `out: &mut Vec<u8>`
//! by deref-coercion into `decompress_into_lz4(out: &mut [u8])`. A
//! `&mut Vec<u8>` deref-coerces to a slice of its current *length*
//! (still 0 after `reserve_exact`), so `lzzzz::lz4::decompress` (and
//! transitively `LZ4_decompress_safe`) is always invoked with a
//! zero-length destination buffer and always fails with
//! `Error::LZ4(DecompressionFailed)` for any non-empty LZ4 payload. This
//! was confirmed to be entirely internal to `ba2`, with zero involvement
//! from `modfather-ba2`'s byte layout: an oracle-native archive, built
//! and read using nothing but `ba2`'s own types
//! (`Chunk::from_decompressed` -> `Archive::write` -> `Archive::read` ->
//! `File::write`), fails the exact same way on its own LZ4 output. The
//! zlib branch (`decompress_into_zlib`) does not have this bug because it
//! writes into the `Vec` via `Write::write_all`, which grows the vec
//! itself instead of relying on a pre-sized destination slice.
//!
//! Because `File::write`/`Chunk::decompress_into` are the *only* public
//! decode entry points `ba2` exposes, and both are broken for LZ4, this
//! test cannot ask the oracle to decode an LZ4 payload through its normal
//! API. Instead, for the LZ4 case only, it decompresses the oracle's
//! *compressed* bytes (`Chunk::as_bytes()`, `Chunk::decompressed_len()`
//! -- both public, unaffected by the bug) directly via `lzzzz::lz4`,
//! which is the same underlying LZ4 implementation `ba2` itself uses, so
//! this is still a decode by "the oracle's own codec", just invoked
//! without going through the broken wrapper.

use ba2::fo4::{
    Archive as OracleArchive, ArchiveKey as OracleArchiveKey, ArchiveOptions as OracleOptions,
    Chunk as OracleChunk, CompressionFormat as OracleCompressionFormat, File as OracleFile,
    FileWriteOptions as OracleFileWriteOptions, Format as OracleFormat, Version as OracleVersion,
};
use ba2::prelude::*;
use modfather_ba2::{write, Ba2Archive, FileToPack, WriteOptions};
use std::io::Cursor;

/// Decode one oracle-read `Chunk`'s bytes, working around the LZ4 bug in
/// `ba2` v3.0.1's `Chunk::decompress_into`/`File::write` documented above.
/// zlib chunks are decoded via the oracle's own public `File::write`; LZ4
/// chunks are decoded by calling `lzzzz::lz4::decompress` directly against
/// the chunk's raw compressed bytes.
fn decode_oracle_file(
    file: &OracleFile,
    codec: OracleCompressionFormat,
    read_options: &OracleFileWriteOptions,
) -> Vec<u8> {
    match codec {
        OracleCompressionFormat::Zip => {
            let mut decoded = Vec::new();
            file.write(&mut decoded, read_options)
                .expect("oracle failed to decode a zlib payload");
            decoded
        }
        OracleCompressionFormat::LZ4 => {
            assert_eq!(file.len(), 1, "GNRL files have exactly one chunk");
            let chunk = &file[0];
            let decompressed_len = chunk
                .decompressed_len()
                .expect("LZ4 chunk should report a decompressed length");
            let mut decoded = vec![0u8; decompressed_len];
            let n = lzzzz::lz4::decompress(chunk.as_bytes(), &mut decoded)
                .expect("lzzzz failed to LZ4-decompress the oracle's own chunk bytes");
            assert_eq!(n, decompressed_len);
            decoded
        }
    }
}

/// Our writer -> the oracle's reader.
///
/// If our writer produced the wrong header size for a given version, or
/// wrote the wrong `compression_method` field for v3, the oracle's own
/// `read_header` (which independently implements the exact same
/// version-dependent layout) would misparse the archive, or -- worse --
/// parse it but decode payloads with the wrong codec. Either failure mode
/// is caught here even though our own reader might (wrongly) accept the
/// same bytes.
#[test]
fn our_writer_is_readable_by_independent_oracle() {
    let files = vec![
        FileToPack {
            name: "Interface\\HUDMenu.swf".to_string(),
            data: b"pretend swf bytes, repeated for compressibility ".repeat(20),
        },
        FileToPack {
            name: "Sound\\fx\\click.wav".to_string(),
            data: b"pretend wav bytes, repeated for compressibility ".repeat(15),
        },
    ];

    // (version, compress, force_lz4_v3, expected oracle version, expected
    // oracle compression format)
    let cases = [
        (1u32, true, false, OracleVersion::v1, OracleCompressionFormat::Zip),
        (2u32, true, false, OracleVersion::v2, OracleCompressionFormat::Zip),
        (3u32, true, false, OracleVersion::v3, OracleCompressionFormat::Zip),
        (3u32, true, true, OracleVersion::v3, OracleCompressionFormat::LZ4),
    ];

    for (version, compress, force_lz4_v3, expect_version, expect_codec) in cases {
        let options = WriteOptions {
            version,
            compress,
            force_lz4_v3,
        };
        let mut buf = Vec::new();
        write(Cursor::new(&mut buf), &files, &options).unwrap_or_else(|e| {
            panic!("our writer failed for v{version} force_lz4_v3={force_lz4_v3}: {e}")
        });

        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), &buf).unwrap();
        let (oracle, meta) = OracleArchive::read(tmp.path()).unwrap_or_else(|e| {
            panic!("oracle rejected our v{version} force_lz4_v3={force_lz4_v3} archive: {e}")
        });
        assert_eq!(
            meta.version(),
            expect_version,
            "oracle-detected version for our v{version} archive"
        );
        assert_eq!(
            meta.compression_format(),
            expect_codec,
            "oracle-detected compression format for our v{version} force_lz4_v3={force_lz4_v3} archive"
        );

        let read_options: OracleFileWriteOptions = meta.into();
        for original in &files {
            let normalized = original.name.replace('/', "\\").to_lowercase();
            let key: OracleArchiveKey = normalized.as_str().into();
            let oracle_file = oracle
                .get(&key)
                .unwrap_or_else(|| panic!("oracle: missing file {}", original.name));

            let decoded = decode_oracle_file(oracle_file, expect_codec, &read_options);
            assert_eq!(
                decoded, original.data,
                "oracle-decoded bytes for {} (v{version}, force_lz4_v3={force_lz4_v3})",
                original.name
            );
        }
    }
}

/// The oracle's writer -> our reader.
///
/// If our reader made a wrong assumption about the header-extension size
/// or about where the codec comes from (e.g. still guessing from version
/// number instead of reading the real v3 `compression_method` field),
/// this would fail even though our own writer's output round-trips
/// through our own reader. This is exactly the bug class the header
/// extension / codec-dispatch fix addressed.
#[test]
fn oracle_writer_is_readable_by_our_reader() {
    let cases = [
        (OracleVersion::v1, OracleCompressionFormat::Zip),
        (OracleVersion::v2, OracleCompressionFormat::Zip),
        (OracleVersion::v3, OracleCompressionFormat::Zip),
        (OracleVersion::v3, OracleCompressionFormat::LZ4),
    ];

    let payload = b"nif model bytes, repeated for compressibility ".repeat(20);

    for (version, compression_format) in cases {
        let chunk = OracleChunk::from_decompressed(payload.as_slice());
        let file: OracleFile = [chunk].into_iter().collect();
        let key: OracleArchiveKey = "meshes\\armor\\iron\\cuirass.nif".into();
        let archive: OracleArchive = [(key, file)].into_iter().collect();

        let options = OracleOptions::builder()
            .format(OracleFormat::GNRL)
            .version(version)
            .compression_format(compression_format)
            .strings(true)
            .build();

        let mut buf = Vec::new();
        archive
            .write(&mut buf, &options)
            .unwrap_or_else(|e| panic!("oracle writer failed for {version:?}/{compression_format:?}: {e}"));

        let mut ours = Ba2Archive::open(Cursor::new(buf)).unwrap_or_else(|e| {
            panic!("our reader should open the oracle's {version:?}/{compression_format:?} archive: {e}")
        });
        let entries = ours.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].name.to_lowercase(),
            "meshes\\armor\\iron\\cuirass.nif"
        );

        let out = ours.read_file(0).unwrap_or_else(|e| {
            panic!("our reader failed to decode {version:?}/{compression_format:?} payload: {e}")
        });
        assert_eq!(
            out, payload,
            "our reader decoded bytes for {version:?}/{compression_format:?}"
        );
    }
}
