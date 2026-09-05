# Crucible — the editor

System-of-systems for everything that edits game data. Depends on **Vestibule only**, one-way. Never depends on ModFather, on another Crucible context, or on any post-v1 tool. If Vestibule needs a fact currently sitting in a Crucible context, ownership moves down to Vestibule and the context subscribes.

Crucible does not talk to disk. It reads bytes and paths through Vestibule: a MOD's referenced archives, an MGE's extracted VCS tree. It never opens a source archive itself and never mutates one.

Native PC process, not a web view. ModFather is a desktop application suite; Crucible contexts are native windows the same way the record editor, NIF editor, and the rest of this document are.

## Context protocol

Every editable file kind gets **one** Crucible context, not one per tool:

1. **Park** — game Instance (identity + version line) is preloaded and idle. No load yet.
2. **Attach** — a right-click on a matching object opens or reuses that context's window.
3. **Stream** — load by relationship, not the whole graph. A neighborhood, not a dump.
4. **Subscribe** — features that touch that file kind register on the context instead of opening their own window or owning their own file I/O.

Switching MGE, unlocking a MOD, or opening a different object of the same kind does not restart the context process. Right-click is object-sensitive: only the actions valid for the clicked object appear, and new actions add to the menu unless a rule says one replaces another.

## Records (xEdit-class) — first specialization

- FormIDs (and TES3 name-ids) are **relational identities**: records are rows, references are typed foreign keys. Not opaque integers in a tree.
- An **override is an instance** of an identity, not a silent replacement. Every MOD that overrides a record is flagged on that identity.
- **Originating masters are edit-locked** ground truth. Mass edit walks the override instances, never the locked original.
- All mutations are a **reversible command stack** (undo/redo), journaled — a capability xEdit itself lacks.
- **Master reassignment** is allowed only when the candidate master maps 100% of the required dependencies: FormID match preferred, EditorID-plus-structure check otherwise. This is how ESL-migration orphans get reattached.
- Unlocking or installing a MOD updates the dependency/override graph in place. No restart.
- "ESM" throughout this document means ESM, ESL, or ESP.

## NIF (NifSkope-class)

- Block-graph editor for meshes.
- **BodySlide, Outfit Studio, ChunkMerge, NIF Optimizer are features of this context**, not separate contexts. Same rule as Records: one owner per file kind.
- Cathedral Assets Optimizer's job splits by kind: mesh conversion is a NIF feature, texture conversion is a Textures feature, archive repacking is Vestibule's job — not one monolithic CAO context.

## Materials

- Bethesda BGSM/BGEM is a **v1 stand-in** specialization.
- Dismounts post-v1 in favor of a substance-class open-source editor (materials/substances/effects as one editor discipline). NIF and Blender subscribe to Materials; they do not own material authoring.

## Textures

- DDS view/convert/mip-chain job. Power-of-two sizes are first-class (mip and typical game-texture rules); arbitrary sizes are allowed, not preferred.
- `texconv`-class and CAO's texture pass subscribe here, not to NIF.
- Post-v1: GIMP becomes the texture tool proper. Its canvas is a 3D planar mesh, i.e. a UV map — compatible with Blender and the substance-class editor. The v1 stand-in dismounts when that project exists.

## Papyrus

- One context for Bethesda scripts, source and compiled: Champollion/Caprica-class decompile/compile.
- Papyrus is a **payload format**, not a scripting surface for ModFather itself. Rhai is that surface (see `VESTIBULE.md` / FOMOD docs for where Rhai runs).

## Animation / behavior

- HKX and behavior-graph editing.
- FNIS / Nemesis / Pandora / OAR / DAR run as **bake jobs** invoked from the UI shell, writing through Vestibule — not as launched external tools with their own file trees.

## LOD pipelines

- xLODGen / DynDOLOD / TexGen-class jobs are **integrator pipelines** subscribed to Records + NIF + Textures. They do not own a mesh or texture browser of their own.

## Localization, audio, UI, saves

- **Localization** — xTranslator-class STRING/ILSTRING/DLSTRING pack editing.
- **Audio** — wrapper stand-ins (Unfuzer/Yakitori-class) in v1; dismounts for Audacity post-v1.
- **UI (SWF)** — JPEXS-class Scaleform editing.
- **Saves** — Fallrim-class ESS/FOS graph editing.

Records can request Localization or Papyrus without importing them as a crate dependency — contexts call each other through the same one-way custody rules Vestibule enforces on file access.

## Extenders and launch

- Each supported game gets a ModFather extender module (naming follows the game: e.g. an F4SE-class, SKSE-class, NVSE-class, FOSE-class, OBSE-class, MWSE-class, SFSE-class loader), versioned with the Instance.
- The game executable is the child process. The extender module is the loader.
- Stock `*_loader.exe` files may exist on disk as probes; they are never selected for launch.
- Bake runs before launch; Overwrite capture is last.
- Rhai plugins run inside the extender module before the game process spawns. Native DLL injection, if it exists, is a later increment of the same module — not a separate product and not in-process game hooking from *this* repo's core.

## Post-v1 tools

Audacity, GIMP, Blender, and a substance-class editor become independent Rust systems and Crucible contexts. All of them depend on Crucible only, never on ModFather directly, matching every other specialization. Their v1 stand-ins (Materials, Textures, Audio) dismount when the real project lands.

## What stays out of Crucible entirely

- Native script-extender internals and Address-Library/CommonLib-class ABI work — those stay in the extender modules under "Extenders and launch," not as a Crucible context.
- ENB family, Community Shaders, Engine Fixes, or any in-process game hooking — out of scope for this repository per the project principles.
- Cloning MO2's USVFS, Vortex, or Wabbajack as products — Vestibule and Crucible take the useful *behavior*, not the executable.
