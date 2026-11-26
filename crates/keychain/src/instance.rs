//! One PingCraft at a time per data folder.
//!
//! Two copies of PingCraft on the same data folder would each keep their own idea of the vault, the profile and the
//! stores, and the last one to write would win: a store key written by one could be lost when the other saved the
//! vault, leaving an encrypted store nobody can open. So the app takes an [`InstanceLock`] on the folder before it
//! touches any of them, and holds it until it exits. A second copy is told that PingCraft is already open.
//!
//! The lock is the operating system's advisory file lock on `<data folder>/lock`, which ends with the process,
//! even when it crashes. The file itself is empty and stays; deleting it while PingCraft runs would let a second
//! copy lock a new file, so nothing ever deletes it.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;

/// The lock file's name in the data folder.
pub const LOCK_FILE: &str = "lock";

/// Why the data folder could not be locked.
#[derive(Debug)]
pub enum LockError {
    /// Another PingCraft has this data folder open.
    Held,
    /// The lock file could not be made or locked, in the platform's words.
    Io(String),
}

impl std::fmt::Display for LockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Held => f.write_str("PingCraft is already open with this data folder"),
            Self::Io(why) => write!(f, "the data folder could not be locked: {why}"),
        }
    }
}

impl std::error::Error for LockError {}

/// The data folder, locked for this process until the value is dropped.
#[derive(Debug)]
pub struct InstanceLock {
    _file: File,
}

impl InstanceLock {
    /// Locks `data_dir` for this process, without waiting: if another process holds it, this fails at once with
    /// [`LockError::Held`].
    pub fn acquire(data_dir: &Path) -> Result<Self, LockError> {
        let io = |e: std::io::Error| LockError::Io(e.to_string());
        std::fs::create_dir_all(data_dir).map_err(io)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(data_dir.join(LOCK_FILE)).map_err(io)?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(TryLockError::WouldBlock) => Err(LockError::Held),
            Err(TryLockError::Error(e)) => Err(io(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{InstanceLock, LockError};

    #[test]
    fn a_second_lock_on_the_same_folder_is_refused_until_the_first_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let first = InstanceLock::acquire(dir.path()).unwrap();
        assert!(matches!(InstanceLock::acquire(dir.path()), Err(LockError::Held)));
        drop(first);
        let again = InstanceLock::acquire(dir.path());
        assert!(again.is_ok(), "the lock ends with its holder: {again:?}");
    }

    #[test]
    fn other_folders_are_not_affected_and_a_missing_folder_is_made() {
        let dir = tempfile::tempdir().unwrap();
        let _a = InstanceLock::acquire(&dir.path().join("a")).unwrap();
        let b = InstanceLock::acquire(&dir.path().join("b"));
        assert!(b.is_ok(), "{b:?}");
    }
}
