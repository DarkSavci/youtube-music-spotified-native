//! Folders and files for tests, deleted when the test is done with them,
//! passed or failed: each run used to leave its own behind in the system's
//! temporary folder.

use std::ops::Deref;
use std::path::{Path, PathBuf};

/// A place in the system's temporary folder that is removed, with all that
/// is in it, when this goes out of scope.
pub struct Scratch {
    /// What the test works with: the place itself or a path inside it.
    path: PathBuf,
    /// What is removed at the end.
    root: PathBuf,
}

impl Scratch {
    /// A fresh, empty folder called `name`; one left by an earlier run goes
    /// first.
    pub fn folder(name: &str) -> Scratch {
        let scratch = Scratch::path(name);
        std::fs::create_dir_all(&scratch.root).expect("a scratch directory");
        scratch
    }

    /// A name with nothing under it yet, for a test that makes a single
    /// file there, or leaves it to the code under test to make.
    pub fn path(name: &str) -> Scratch {
        let root = std::env::temp_dir().join(name);
        remove(&root);
        Scratch {
            path: root.clone(),
            root,
        }
    }

    /// The same place, handing out `child` inside it instead: the whole
    /// place is still what is removed.
    pub fn at(mut self, child: impl AsRef<Path>) -> Scratch {
        self.path = self.root.join(child);
        self
    }
}

impl Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        remove(&self.root);
    }
}

/// Removes a folder or a file, whichever is there; nothing being there
/// is fine.
fn remove(path: &Path) {
    if path.is_dir() {
        let _ = std::fs::remove_dir_all(path);
    } else {
        let _ = std::fs::remove_file(path);
    }
}
