# MGE — ModFather Game Environment

An MGE is the playable environment. A MOD is the downloaded family.

- References MODs. Does not copy `.mod` packages.
- Records how those references are enabled (lock, FOMOD selection, order).
- Stores the file structure of activated components as **one** loose-file tree. Never a second loose collection.
- Switch MGE without restarting ModFather. Vestibule remounts live.
- Version-controlled. That version limits the hierarchy: it pins MOD identities, file versions, and selections.
- Themeable (collection analogue). An MGE can be saved as a theme.
- A new MGE lists every viable downloaded MOD **Locked**.
