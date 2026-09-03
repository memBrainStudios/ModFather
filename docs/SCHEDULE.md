# ModFather v1 schedule

ModFather is a **native PC application suite** (Rust), not a web app. Waves are relative order, not dates. A wave starts once the previous wave's gate passes.

Custody for every wave: Vestibule owns files; Crucible owns the domain/editor framework; the ModFather shell owns UI and launch; specializations keep only their own domain work; dependencies are one-way (`7-Zip RE → Vestibule → Crucible → ModFather`); every Bethesda-ecosystem tool named in the README/docs is in scope for v1. GIMP / Audacity / Blender / a substance-class editor are v1+ and appear only as sockets (stand-ins in Crucible dismount when those land).

"ESM" = ESM / ESL / ESP throughout.

## Wave 0 — Vestibule floor

- **7-Zip RE standalone** (`sevenzip-re`): native Rust 7z container read/write with Copy/LZMA/LZMA2 codecs. No Bethesda code, no host `7z` binary, no game dependency. Independently redistributable.
- **Bethesda archive extensions** (separate crates, e.g. `modfather-bsa`, `modfather-ba2`): BSA (v103–105) and BA2 (GNRL, DX10) read/write, registered as additional container handlers alongside 7z — never folded into the standalone package. Pack per Bethesda doctrine (see `VESTIBULE.md`).
- **LOOT**: generalized sorter stub — Nexus categories, traditional masterlist, user rules — even if the rule engine is thin at first.
- **Vestibule** VFS roots, last-wins layering, compare tooltips.

**Gate:** round-trip a real 7z archive through the standalone package alone (no BSA/BA2 dependency); separately, list + extract a real BSA and a real BA2 through the extension crates; pack a stem back to `{stem}.bsa` / `{stem} - Textures.bsa` and `{stem} - Main.ba2` / `{stem} - Textures.ba2`; a manual conflict pick overrides last-wins on one path.

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
