//! VFS roots: the three real on-disk locations Vestibule reasons about
//! (per `docs/VESTIBULE.md` / `docs/SCHEDULE.md`'s Wave 0 floor).
//!
//! This is deliberately thin for Wave 0: just enough structure to name and
//! validate the roots a game Instance needs, so [`crate::layering`] has a
//! concrete `Root` to resolve paths under. Populating these from a real
//! game Instance (detecting the install dir, save/config location, etc.)
//! is a later-wave concern.

use std::path::PathBuf;

/// Which real filesystem location a VFS path resolves under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Root {
    /// The game's install directory (executables, stock `*_loader.exe`
    /// probes, engine files).
    Install,
    /// Save games and user config (INI files, save files).
    SaveConfig,
    /// The game's `Data` directory: where BSA/BA2 archives and loose
    /// assets ultimately layer together for the running game.
    Data,
}

/// A game Instance's three VFS roots, resolved to real paths on disk.
#[derive(Debug, Clone)]
pub struct VfsRoots {
    pub install: PathBuf,
    pub save_config: PathBuf,
    pub data: PathBuf,
}

impl VfsRoots {
    pub fn new(install: impl Into<PathBuf>, save_config: impl Into<PathBuf>, data: impl Into<PathBuf>) -> Self {
        VfsRoots {
            install: install.into(),
            save_config: save_config.into(),
            data: data.into(),
        }
    }

    /// Resolve a [`Root`] to its real path for this Instance.
    pub fn path_for(&self, root: Root) -> &std::path::Path {
        match root {
            Root::Install => &self.install,
            Root::SaveConfig => &self.save_config,
            Root::Data => &self.data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_each_root_independently() {
        let roots = VfsRoots::new("/games/Skyrim", "/home/user/My Games/Skyrim", "/games/Skyrim/Data");
        assert_eq!(roots.path_for(Root::Install), std::path::Path::new("/games/Skyrim"));
        assert_eq!(
            roots.path_for(Root::SaveConfig),
            std::path::Path::new("/home/user/My Games/Skyrim")
        );
        assert_eq!(roots.path_for(Root::Data), std::path::Path::new("/games/Skyrim/Data"));
    }
}
