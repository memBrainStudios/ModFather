//! Wave 0 gate: round-trip a real 7z archive through the standalone
//! package alone (no BSA/BA2 dependency).

use sevenzip_re::{create, Archive, NewEntry, PackCodec};

#[test]
fn create_and_read_back_copy() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.7z");

    let entries = vec![
        NewEntry {
            name: "hello.txt".to_string(),
            data: b"Hello, ModFather!".to_vec(),
        },
        NewEntry {
            name: "dir/nested.txt".to_string(),
            data: b"Nested content for the Wave 0 gate test.".to_vec(),
        },
    ];

    create(&path, &entries, PackCodec::Copy).unwrap();

    let mut archive = Archive::open(&path).unwrap();
    let listed = archive.entries();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].name, "hello.txt");
    assert_eq!(listed[1].name, "dir/nested.txt");

    let content0 = archive.read_file("hello.txt").unwrap();
    assert_eq!(content0, b"Hello, ModFather!");

    let content1 = archive.read_file("dir/nested.txt").unwrap();
    assert_eq!(content1, b"Nested content for the Wave 0 gate test.");
}

#[test]
fn create_and_read_back_lzma() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t_lzma.7z");

    let payload: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
    let entries = vec![NewEntry {
        name: "data.bin".to_string(),
        data: payload.clone(),
    }];

    create(&path, &entries, PackCodec::Lzma).unwrap();

    let mut archive = Archive::open(&path).unwrap();
    let out = archive.read_file("data.bin").unwrap();
    assert_eq!(out, payload);
}

#[test]
fn create_and_read_back_lzma2() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t_lzma2.7z");

    let payload: Vec<u8> = b"repeat repeat repeat repeat repeat repeat repeat repeat!"
        .iter()
        .cycle()
        .take(8000)
        .copied()
        .collect();
    let entries = vec![NewEntry {
        name: "data2.bin".to_string(),
        data: payload.clone(),
    }];

    create(&path, &entries, PackCodec::Lzma2).unwrap();

    let mut archive = Archive::open(&path).unwrap();
    let out = archive.read_file("data2.bin").unwrap();
    assert_eq!(out, payload);
}

/// Cross-check against a real 7z archive produced by the system `7z`
/// binary. This uses the system binary only as a **test fixture generator**
/// — `sevenzip-re` itself never shells out to `7z`; this is exactly the
/// anti-pattern the standalone package replaces.
#[test]
fn read_real_7z_fixture_created_by_system_binary() {
    let seven_zip = which_7z();
    let Some(seven_zip) = seven_zip else {
        eprintln!("skipping: no system 7z/7za binary found for fixture generation");
        return;
    };

    let dir = tempfile::tempdir().unwrap();
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("a.txt"), b"fixture file A").unwrap();
    std::fs::write(src_dir.join("b.txt"), b"fixture file B, a bit longer than A").unwrap();

    let archive_path = dir.path().join("fixture.7z");
    let status = std::process::Command::new(&seven_zip)
        .arg("a")
        .arg(archive_path.to_str().unwrap())
        .arg(".")
        .current_dir(&src_dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("failed to spawn system 7z for fixture generation");
    assert!(status.success(), "system 7z failed to create fixture");

    let mut archive = Archive::open(&archive_path).unwrap();
    let mut listed = archive.entries();
    listed.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].name, "a.txt");
    assert_eq!(listed[1].name, "b.txt");

    let a = archive.read_file("a.txt").unwrap();
    assert_eq!(a, b"fixture file A");
    let b = archive.read_file("b.txt").unwrap();
    assert_eq!(b, b"fixture file B, a bit longer than A");
}

/// The other direction of the same cross-check: an archive **we** create
/// must be readable by the real system `7z` binary, not just by our own
/// reader. This closes the one-way gap `docs/SCHEDULE.md`'s Wave 0 gate
/// note flags for `read_real_7z_fixture_created_by_system_binary` above
/// ("external `7z` writes, our reader reads; no packing-side real-archive
/// fixture yet"). As with that test, the system binary is used only as an
/// independent **verifier** here (`7z t` integrity test, then `7z x`
/// extract-and-byte-compare) — `sevenzip-re`'s own `create()` never shells
/// out to it.
#[test]
fn our_writer_is_readable_by_the_system_binary() {
    let seven_zip = which_7z();
    let Some(seven_zip) = seven_zip else {
        eprintln!("skipping: no system 7z/7za binary found for verification");
        return;
    };

    for codec in [PackCodec::Copy, PackCodec::Lzma, PackCodec::Lzma2] {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("ours.7z");

        // A payload with enough internal repetition that LZMA/LZMA2 both
        // produce a genuinely compressed (not larger-than-input) stream,
        // exercising the real codec path rather than a degenerate
        // near-empty case.
        let payload: Vec<u8> = b"ModFather 7-Zip RE writer cross-check payload. "
            .iter()
            .cycle()
            .take(4096)
            .copied()
            .collect();

        let entries = vec![
            NewEntry {
                name: "top.txt".to_string(),
                data: b"top-level entry".to_vec(),
            },
            NewEntry {
                name: "nested/deep.bin".to_string(),
                data: payload.clone(),
            },
        ];
        create(&archive_path, &entries, codec).unwrap();

        // 1. `7z t`: full integrity test (CRCs, header parsing) by an
        //    entirely independent implementation of the format.
        let test_status = std::process::Command::new(&seven_zip)
            .arg("t")
            .arg(&archive_path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .expect("failed to spawn system 7z to test our archive");
        assert!(
            test_status.success(),
            "system 7z rejected our own {codec:?}-encoded archive as corrupt/invalid"
        );

        // 2. `7z x`: actually extract with the system binary and compare
        //    bytes, proving round-trip correctness end to end, not just a
        //    header-level integrity pass.
        let extract_dir = dir.path().join("extracted");
        std::fs::create_dir_all(&extract_dir).unwrap();
        let extract_status = std::process::Command::new(&seven_zip)
            .arg("x")
            .arg(&archive_path)
            .arg(format!("-o{}", extract_dir.display()))
            .arg("-y")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .expect("failed to spawn system 7z to extract our archive");
        assert!(
            extract_status.success(),
            "system 7z failed to extract our own {codec:?}-encoded archive"
        );

        let top = std::fs::read(extract_dir.join("top.txt")).unwrap();
        assert_eq!(top, b"top-level entry");
        let nested = std::fs::read(extract_dir.join("nested").join("deep.bin")).unwrap();
        assert_eq!(nested, payload, "{codec:?}: extracted bytes must match the original payload exactly");
    }
}

fn which_7z() -> Option<String> {
    for candidate in ["7z", "7za", "7zr"] {
        if std::process::Command::new(candidate)
            .arg("--help")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success() || s.code() == Some(0) || s.code() == Some(1))
            .unwrap_or(false)
        {
            return Some(candidate.to_string());
        }
    }
    None
}
