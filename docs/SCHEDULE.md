# ModFather v1 schedule

ModFather is a **native PC application suite** (Rust), not a web app. Waves are relative order, not dates. A wave starts once the previous wave's gate passes.

Custody for every wave: Vestibule owns files; Crucible owns the domain/editor framework; the ModFather shell owns UI and launch; specializations keep only their own domain work; dependencies are one-way (`7-Zip RE → Vestibule → Crucible → ModFather`); every Bethesda-ecosystem tool named in the README/docs is in scope for v1. GIMP / Audacity / Blender / a substance-class editor are v1+ and appear only as sockets (stand-ins in Crucible dismount when those land).

"ESM" = ESM / ESL / ESP throughout.

## Wave 0 — Vestibule floor

- **7-Zip RE standalone** (`sevenzip-re`): native Rust 7z container read/write with Copy/LZMA/LZMA2 codecs. No Bethesda code, no host `7z` binary, no game dependency. Independently redistributable.
- **Bethesda archive extensions** (separate crates, e.g. `modfather-bsa`, `modfather-ba2`): BSA (v103–105) and BA2 (GNRL, DX10) read/write, registered as additional container handlers alongside 7z via a real `ContainerFormat`/`Registry` mechanism (see the "Modular container-format registry" note below) — never folded into the standalone package. Pack per Bethesda doctrine (see `VESTIBULE.md`).
- **LOOT**: generalized sorter stub — Nexus categories, traditional masterlist, user rules — even if the rule engine is thin at first.
- **Vestibule** VFS roots, last-wins layering, compare tooltips.

**Gate:** round-trip a real 7z archive through the standalone package alone (no BSA/BA2 dependency); separately, list + extract a real BSA and a real BA2 through the extension crates; pack a stem back to `{stem}.bsa` / `{stem} - Textures.bsa` and `{stem} - Main.ba2` / `{stem} - Textures.ba2`; a manual conflict pick overrides last-wins on one path.

**Gate status (in progress):**
- 7z round trip (`sevenzip-re`): synthetic-fixture round trips (Copy/LZMA/LZMA2) pass; both directions of a real-binary cross-check now pass: (1) a real 7z archive produced by the system `7z` binary is read back correctly (`read_real_7z_fixture_created_by_system_binary`), and (2) an archive **we** create (all three codecs: Copy/LZMA/LZMA2) is verified by the system binary itself via `7z t` (integrity test) and `7z x` (extract + byte-compare) (`our_writer_is_readable_by_the_system_binary`) -- this closes the previously one-way gap. As with the reader-side test, the system binary is used only as an independent verifier/fixture generator; `sevenzip-re` itself never shells out to it.
  - This packing-side cross-check immediately caught a real bug in the LZMA2 writer: the dictionary-size coder-property byte was written as the literal `0x28u8` (hex, i.e. decimal 40 -- the LZMA2 spec's "unbounded/4 GiB" sentinel value) when the intent (per the comment beside it, and confirmed against the real LZMA2 property-byte formula) was decimal 28 (64 MiB, a normal bounded size). Real 7-Zip refused to open our own output with "Can't allocate required memory!" because it took the sentinel literally and tried to reserve a 4 GiB window. `sevenzip-re`'s own `decode_lzma2` was unaffected (the pure-Rust decoder sizes its window from the LZMA2 stream's own chunk headers, not this property byte), which is exactly why this bug was invisible to every prior same-crate round-trip test and was only caught once a *second*, independent LZMA2 implementation (the real `7z` binary) was asked to open our output. Fixed in `crates/sevenzip-re/src/codec.rs::encode_lzma2`.
- BSA/BA2 real-archive fixtures: genuine Bethesda-game-shipped BSA/BA2 files are copyrighted game assets and are not available in this sandbox (no licensed game install). Real-world validation is unblocked without needing licensed archives by using the independently-written, 0BSD-licensed `ba2` crate (crates.io) as a **dev-only test oracle** in both `modfather-bsa` and `modfather-ba2` (`tests/oracle_cross_validation.rs` in each crate; never a runtime dependency). Both directions (our writer → oracle reader, oracle writer → our reader) pass for BSA (v103/104/105, zlib and LZ4) and for BA2 (v1/v2/v3, zlib and LZ4).
  - This process found and fixed four real structural bugs, all now covered by regression tests:
    1. **BSA v105 LZ4 framing** (`modfather-bsa`): the reader/writer used LZ4 *frame* format instead of the real *block* format Skyrim SE actually uses.
    2. **BA2 v2/v3 header extension** (`modfather-ba2`): the base 24-byte BA2 header is only correct for v1/v7/v8; v2 needs +8 reserved bytes, v3 needs +8 reserved bytes plus a genuine per-archive `compression_method: u32` field. The old code always assumed the fixed 24-byte header.
    3. **BA2 v2/v3 codec dispatch** (`modfather-ba2`): v2 was wrongly treated as always-LZ4 (it's always zlib); v3's codec was guessed from the version number instead of read from the real per-archive `compression_method` field.
    4. **BA2 GNRL file-record `chunk_size`/`numChunks` fields** (`modfather-ba2`): the writer wrote these as a blind zero `u32`, but real readers validate them as `numChunks=1, chunkHeaderSize=0x10` for GNRL files.
  - Also found (and worked around in the test, not a ModFather bug): `ba2` crate v3.0.1's own `Chunk::decompress_into` is broken for its LZ4 branch (`out.reserve_exact(len)` only grows capacity, not length, before deref-coercing to a zero-length destination slice) -- confirmed via an oracle-only, zero-ModFather-code repro. Documented in `modfather-ba2/tests/oracle_cross_validation.rs`; worked around by decompressing the oracle's own compressed bytes directly via `lzzzz::lz4` for that one case.
- LOOT generalized-sorter stub (`modfather-vestibule::loot`): implemented per `docs/VESTIBULE.md`'s "Nexus categories (user may use or modify them), traditional masterlist, and user-created rules" -- deliberately a **thin rule engine** for Wave 0 (flat masterlist priority list, pairwise user `before`/`after` constraints, not LOOT's full metadata/condition language). Precedence, highest first: user rules (applied as a stable topological adjustment so a rule only moves the plugins it actually constrains) > masterlist rank > Nexus category priority > original input order as the final stable tiebreaker. Cyclic user rules and rules naming an unknown plugin are rejected as errors rather than silently ignored or panicking. 10 unit tests cover masterlist precedence, category fallback, rule precedence/locality, cycles, unknown-plugin rules, case-insensitive name matching, and the empty-input edge case.
- Real pull-pipeline wiring (`download-repo` -> `VCS`) not started; that is Wave 1 scope (`MOD`/`MGE` state containers), not part of the Wave 0 floor itself. Gate is not yet fully closed pending a genuine licensed-archive spot check (offered by the project owner, not yet performed) -- not required to close the gate given the oracle cross-validation above, but tracked as a future nice-to-have. **Not related to RAR** (RAR extraction is a separate, unambiguously-documented placeholder pending license -- see `VESTIBULE.md`'s "7-Zip RE is two deliverables" section -- with no bearing on this line): this line is specifically about validating BSA/BA2 against a real, licensed game archive instead of only synthetic fixtures + the independent `ba2` crate oracle.
- **Modular container-format registry** (`sevenzip_re::container`, `modfather_bsa::container`, `modfather_ba2::container`, `modfather_7zre::container`): the "registered as additional container handlers alongside 7z" line above used to be prose only -- `sevenzip_re::Archive`, `modfather_bsa::BsaArchive`, and `modfather_ba2::Ba2Archive` were (and remain) three independent concrete types with no shared trait or dispatch mechanism between them. Closed with a real GoF Strategy (`ContainerFormat`/`ContainerHandle`, one implementation per format, wrapping each crate's existing concrete archive type without changing its public API) + Factory (`Registry`, which peeks header bytes and dispatches to whichever registered format's `probe` matches) pair, defined once in `sevenzip_re::container` and never pre-populated there (that would mean the standalone package depending on Bethesda-specific crates, inverting the one-way custody chain). `modfather_7zre::container::build_registry` is where 7z + BSA + BA2 are actually assembled into one shared `Registry`. This assembly used to live in `modfather-vestibule::container` but moved to a new crate, `modfather-7zre`, per the project's later architecture decision ("Vestibule is a client of 7-Zip RE... at no time does Vestibule implement anything related to an archive") -- `modfather-7zre` sits between the format crates and Vestibule in the custody chain specifically so it, not Vestibule, is the crate that already depends on every format extension. A future RAR `ContainerFormat` (once licensed) registers the same way, with one added line and no changes to any existing caller.
- **TES3 (Morrowind) BSA**: decided and implemented. Per the project's architecture rule ("each format should contain every version named as such... if Morrowind is the oldest version of BSA, it is the ground truth for the architecture"), `modfather-bsa` is now split into two version-named submodules: `tes3` (Morrowind -- uncompressed, no folder records, its own 64-bit path hash, magic `0x0000_0100`) and `tes4` (Oblivion through Skyrim SE/AE, v103-105, sharing one reader/writer since those versions differ only by codec/padding, magic `b"BSA\0"`). The two families are told apart purely by header magic bytes via two `sevenzip_re::container::ContainerFormat` strategies (`container::Tes3BsaFormat` / `container::Tes4BsaFormat`), both registered in the same `Registry` under the same `"bsa"` name -- no file extension or caller hint needed. `modfather_bsa::{BsaArchive, FileToPack, WriteOptions, write}` remain re-exported at the crate root from `tes4` for backward compatibility with existing callers (e.g. `modfather-7zre::packing`, formerly `modfather-vestibule::packing` before the container/packing/dds migration below), which predate this split and only ever meant the tes4 family.
  - TES3's binary layout and hash algorithm were cross-checked against three independent sources: the UESP wiki spec, nifskope's `bsa.{h,cpp}` reference reader (which resolved a non-obvious detail the UESP prose alone left ambiguous -- stored file offsets are relative to `dataOffset = 12 + hashOffset + fileCount*8`, not absolute), and the independently-written `ba2` crate's own `tes3` module, whose 10 published known-answer hash vectors this crate's `tes3::hash::hash_path` matches exactly.
  - Both directions of oracle cross-validation pass (`modfather-bsa/tests/tes3_oracle_cross_validation.rs`, mirroring the existing tes4 oracle test): our writer's output is readable by `ba2::tes3::Archive`, and `ba2::tes3::Archive`'s output is readable by our reader.
  - The writer rejects genuine hash collisions between two distinct paths as a hard error (`Error::HashCollision`) rather than silently dropping an entry, per UESP's documented real-tooling behavior; covered by a regression test using two differently-cased spellings of the same normalized path (a cheap, deterministic way to force a collision).
  - BA2 was deliberately **not** given the same tes-version-named-submodule treatment: its v1/v2/v3/v7/v8 generations already share one reader/writer/format module, version-gated by branches rather than a structural rewrite the way Oblivion replaced Morrowind's BSA layout -- consistent with the architecture rule's "where feasible, share code" half. This crate has no Morrowind-era structural break to partition around.
- **Composed gate test** (`crates/modfather-vestibule/tests/wave0_gate.rs::packed_bsa_and_ba2_archives_compose_through_layering_with_a_manual_pick`): everything above proves each gate clause in isolation, in the crate that owns that clause -- but nothing previously proved the clauses *compose* through Vestibule, the crate with actual file custody. This test packs two synthetic MODs (one to BSA, one to BA2) via `modfather_7zre::packing::pack_bsa_stem`/`pack_ba2_stem`, lists + extracts the real resulting archives back out through each crate's own reader (not a pre-canned fixture), feeds the recovered VFS paths into `layering::resolve`, and confirms last-wins picks the later MOD on a genuinely conflicting shared path (different real packed-and-reread bytes, not just a name collision) while a manual pick flips only that one path back, leaving both MODs' unique paths untouched -- the gate's exact wording end to end.
- **DX10 (texture-chunk) BA2 writer** (`modfather-ba2::writer::write_dx10`): previously the top-listed remaining gap -- `pack_ba2_stem`'s Textures archives were always packed as GNRL, never real DX10, because no DX10 writer existed at all (only the read side, `reader::read_dx10_entries`, was implemented). Closed: `write_dx10` emits the exact `F4TexInfo`(24 bytes)+`F4TexChunk`(24 bytes) layout the reader already parses, Wave-0 scope = exactly one chunk per texture spanning the full mip range (`start_mip=0`, `end_mip=num_mips-1`), rather than the up to 4 independently-streamable chunks real Archive2 output may use for very large textures -- tracked as further follow-up if multi-chunk streaming is ever actually needed; a single full-range chunk is still a structurally valid DX10 file this crate's own reader (and the independent oracle, see below) round-trips correctly, since chunk count is a real per-file field, never assumed to be 1.
  - Cross-validated both directions against the independent `ba2` crate oracle (`modfather-ba2/tests/oracle_cross_validation.rs::our_dx10_writer_is_readable_by_independent_oracle` / `oracle_dx10_writer_is_readable_by_our_reader`), bypassing DDS/DirectXTex parsing on the oracle side entirely (building/reading its `File`/`Chunk` types directly) since decoding real `.dds` pixel data is out of scope for this archive-layout check.
  - `modfather_7zre::packing::pack_ba2_stem` now actually routes `.dds` Textures files through `write_dx10` instead of GNRL, via a new minimal DDS-header parser (`modfather_7zre::dds`, deliberately not a full DDS/DXGI implementation -- see that module's doc comment) that extracts just the four `F4TexInfo` metadata fields (`height`/`width`/`num_mips`/`format`) and strips the header off so only raw mip bytes get packed, matching what real Archive2 output packs. This closes the "Known limitation" this module's doc comment used to flag.
- **Vestibule is a pure client of 7-Zip RE**: per the project's architecture decision ("Vestibule is a client of 7-Zip RE. 7-Zip RE is responsible for all archives related to this project... at no time does Vestibule implement anything related to an archive"), `container.rs`/`packing.rs`/`dds.rs` moved out of `modfather-vestibule` into a new crate, `modfather-7zre`, which depends on `sevenzip-re` + `modfather-bsa` + `modfather-ba2` (so it, not `sevenzip-re` itself, can own registry assembly and Bethesda-specific packing without inverting the no-Bethesda-dependency rule on the standalone package). `modfather-vestibule` now depends only on `modfather-7zre`, never on the format crates directly, in its own (non-test) code; its remaining modules (`layering`, `loot`, `vfs`) had zero archive-format dependencies to begin with (confirmed by grep before the move) and are unchanged. `wave0_gate.rs` still reads packed archives back with `modfather-bsa`'s/`modfather-ba2`'s own readers to verify `modfather-7zre`'s output is real and spec-conforming, but those crates are now dev-dependencies of `modfather-vestibule`, not runtime dependencies.

---

## Wave 1 — MOD and MGE as state containers

- `MOD` object: identity resolution (`nexus:{gameDomain}:{modId}` → other store id → `hash:{sha256}`), references to archives, per-MGE component settings, no embedded archive bytes.
- `MGE` object: lock slider per MOD + order, last-click Active (only if Unlocked), extracted VCS tree, conflict overlay.
- Pull pipeline: `download-repo/<MOD name>/` (archives stay archives) → `VCS/mods/<MOD name>/<archive name>/` (extract once per hash) → packable loose auto-packed, mandatory loose stays in `.../loose/`.
- New MGE: every viable downloaded MOD starts Locked.

**Gate:** pull a fixture MOD, see it land in download-repo then VCS; unlock it in one MGE and confirm a second MGE still shows it Locked; click sets Active and opens FOMOD without extracting anything new.

---

## Wave 2 — FOMOD as UI, derived options

- Parse `ModuleConfig.xml` (UTF-8 and UTF-16) when present.
- Derive options from the nested archive tree when it is absent: one group per archive folder.
- Radio buttons where candidates share a destination path; checkboxes where they do not.
- FOMOD selection is the access key into that MOD's VCS tree — selection changes visibility, not bytes on disk.
- Compatibility rules: external MOD not present → pull request generated and resolved; present and inactive → request to auto-activate (Unlock, then Active).
- Collections batch that walk; more than one collection may run in a session, sharing the same download-repo, VCS, and conflict overlay.

**Gate:** a real FOMOD package (radios + checkboxes) resolves to the same file set the official installer would produce; a derived-option MOD (no ModuleConfig.xml) presents one group per archive folder; a collection with a missing dependency generates a pull request and, once resolved, auto-activates it.

---

## Wave 3 — Bash: conflicts across MODs

- Automatic bash of plugin records and loose assets among Unlocked MODs (last-file-served) when the player has not picked winners.
- Player-selected conflict winners override automatic bash.
- Non-conflicting extracts stay in their MOD/archive folder; only conflict resolutions land in an MGE's loose overlay. Never a second loose collection.

**Gate:** three fixture MODs with overlapping loose paths bash to last-file-served by default; setting one explicit winner changes only that path.

---

## Wave 4 — Crucible: context protocol and Records

Stand up Crucible as its own system-of-systems, depending on Vestibule only.

- Context protocol: park / attach / stream / subscribe.
- Records context (xEdit-class): FormID-relational model, override-as-instance, journaled reversible edits, mass edit across instances, master reassignment when dependency mapping is 100%.
- Originating masters stay edit-locked ground truth.

**Gate:** right-click an ESM opens a parked Records context; stream one neighborhood without loading the full graph; unlocking a second MOD updates override flags without restarting the context.

---

## Wave 5 — NIF context and mesh features

- NIF context (NifSkope-class): block-graph view/edit.
- BodySlide / Outfit Studio / ChunkMerge / NIF Optimizer as **features** of this context, not separate contexts.
- Cathedral Assets Optimizer's job splits across NIF (mesh), Textures (wave 6), and Vestibule (archive) — never one monolithic context.

**Gate:** right-click a `.nif` opens the NIF context; a BodySlide-class batch job runs as a command on that same context; no second mesh stack exists anywhere in the tree.

---

## Wave 6 — Materials, Textures, Papyrus

- Materials: BGSM/BGEM v1 stand-in (dismounts post-v1 for a substance-class editor).
- Textures: DDS view/convert/mip job; power-of-two preferred, arbitrary allowed; `texconv`/CAO texture pass subscribe here.
- Papyrus: one context for Bethesda scripts (source + compiled), Champollion/Caprica-class compile/decompile.

**Gate:** right-click `.bgsm`, `.dds`, `.pex` each open the matching context; a NIF material slot calls Materials rather than owning its own material editor.

---

## Wave 7 — Animation and LOD

- Animation context for HKX/behavior graphs.
- FNIS / Nemesis / Pandora / OAR / DAR run as bake jobs from the shell, writing through Vestibule.
- xLODGen / DynDOLOD / TexGen as integrator pipelines subscribed to Records + NIF + Textures, owning no browser of their own.

**Gate:** one bake step produces animation output that lands correctly in the sequence/Downloads split; an LOD job runs without opening a private mesh browser.

---

## Wave 8 — Localization, audio, UI, saves

- Localization: xTranslator-class STRING/ILSTRING/DLSTRING packs.
- Audio: v1 wrapper stand-in (dismounts post-v1 for Audacity).
- UI: JPEXS-class SWF editing.
- Saves: Fallrim-class ESS/FOS graph editing.

**Gate:** right-click on each file kind opens only that context's menu; Records can request Localization or Papyrus without a crate-level dependency on either.

---

## Wave 9 — Extenders and launch

- Per-game ModFather extender module (F4SE/SKSE/NVSE/FOSE/OBSE/MWSE/SFSE-class), versioned with the Instance, loading before the game process spawns.
- Stock `*_loader.exe` files remain on disk only as probes — never selected for launch.
- Bake runs first; Overwrite capture is last; Rhai plugins run in the extender module pre-spawn.
- Runtime capture splits into Overwrite (generic) vs. catalog-classified Downloads (official DLC / Creations-class content).

**Gate:** launch a fixture game Instance through its extender module without restarting the shell; a dummy captured file classifies correctly to Overwrite or Downloads.

---

## Wave 10 — Shell chrome and fetch

- Native desktop shell window (Rust GUI, not a browser/Electron/web stack) hosting the MOD/MGE views, FOMOD panels, and Crucible context launch points.
- Context-sensitive right-click menus exactly as specified per wave.
- Nexus-class fetch into an existing or new MOD by id (first file creates it, later files join it).
- Collection apply stays refs-only — no archive bytes copied into a collection file.

**Gate:** click a MOD shows its settings; the lock slider excludes it from bake; "reveal in explorer" works; fetch-by-id joins the same MOD an existing pull created.

---

## Explicitly after v1

GIMP (planar-UV canvas, power-of-two first-class), Audacity, Blender (meshes only — textures are GIMP's job), and a substance-class materials/effects editor. All v1 stand-ins in waves 5–8 dismount when the corresponding project lands. Remaster Instances (e.g. Oblivion Remastered) wait for their asset sets to exist.

## Suggested order of attention

Waves 0–3 (Vestibule + MOD/MGE + FOMOD + bash) are the critical path — nothing in Crucible has anywhere to attach without them. Waves 4–5 (Records, NIF) can overlap once the context protocol is stable. Waves 6–8 are further Crucible specializations and can proceed in parallel once Records/NIF land. Wave 9 (extenders/launch) depends only on Vestibule's bake sequence, not on every Crucible context. Wave 10 (shell chrome) can start a thin frame as soon as wave 1's MOD/MGE state exists, but must not precede Vestibule.
