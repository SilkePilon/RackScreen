//! Files written as root into a user's directory (under `sudo`): the play
//! request and the GIF cache. Created fresh, never through a link, and
//! owned by that user.

use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use anyhow::{bail, Context, Result};

/// A `(uid, gid)` to hand files to.
pub type Owner = (u32, u32);

/// Create `dir` and its missing parents; with `owner`, each directory
/// created here is chowned to it (ones that existed are left alone).
pub fn create_dir_all_owned(dir: &Path, owner: Option<Owner>) -> Result<()> {
    let mut missing = Vec::new();
    let mut d = Some(dir);
    while let Some(p) = d.filter(|p| !p.exists()) {
        missing.push(p.to_path_buf());
        d = p.parent();
    }
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    if let Some((uid, gid)) = owner {
        for p in missing.iter().rev() {
            std::os::unix::fs::chown(p, Some(uid), Some(gid))
                .with_context(|| format!("chown {}", p.display()))?;
        }
    }
    Ok(())
}

/// Write `bytes` to `path` via `tmp` and a rename, so a reader never sees
/// half a file. It never follows a link: a link at `path` is refused, a
/// stale regular `tmp` is removed (anything else there is an error), `tmp`
/// is created fresh with `mode` and, with `owner`, chowned through its
/// handle before the rename.
pub fn write_new_owned(
    path: &Path,
    tmp: &Path,
    bytes: &[u8],
    mode: u32,
    owner: Option<Owner>,
) -> Result<()> {
    if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        bail!("{} is a symlink; not writing through it", path.display());
    }
    match std::fs::symlink_metadata(tmp) {
        Ok(m) if m.file_type().is_file() => {
            std::fs::remove_file(tmp).with_context(|| format!("remove stale {}", tmp.display()))?
        }
        Ok(_) => bail!("{} exists and is not a regular file", tmp.display()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e).with_context(|| format!("stat {}", tmp.display())),
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(tmp)
        .with_context(|| format!("create {}", tmp.display()))?;
    file.write_all(bytes)
        .with_context(|| format!("write {}", tmp.display()))?;
    if let Some((uid, gid)) = owner {
        std::os::unix::fs::fchown(&file, Some(uid), Some(gid))
            .with_context(|| format!("chown {}", tmp.display()))?;
    }
    drop(file);
    std::fs::rename(tmp, path).with_context(|| format!("rename to {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;

    /// Whoever we run as: the owner of a directory we just made.
    fn me() -> Owner {
        let d = tmpdir();
        let m2 = std::fs::metadata(&d).unwrap();
        let _ = std::fs::remove_dir_all(&d);
        (m2.uid(), m2.gid())
    }

    fn tmpdir() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "rackscreen-owned-{}-{}",
            std::process::id(),
            fastrand::u64(..)
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn creates_missing_dirs_and_writes_via_a_fresh_temp_file() {
        let root = tmpdir();
        let dir = root.join("a").join("b");
        create_dir_all_owned(&dir, Some(me())).unwrap();
        assert!(dir.is_dir());
        let path = dir.join("f");
        let tmp = dir.join("f.tmp");
        write_new_owned(&path, &tmp, b"one", 0o600, Some(me())).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"one");
        let m = std::fs::metadata(&path).unwrap();
        assert_eq!((m.uid(), m.gid()), me());
        assert_eq!(m.mode() & 0o777, 0o600);
        assert!(!tmp.exists());
        // a stale regular temp file is replaced
        std::fs::write(&tmp, "stale").unwrap();
        write_new_owned(&path, &tmp, b"two", 0o644, None).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn never_writes_through_a_link() {
        let root = tmpdir();
        let victim = root.join("victim");
        std::fs::write(&victim, "keep me").unwrap();
        let (path, tmp) = (root.join("f"), root.join("f.tmp"));
        std::os::unix::fs::symlink(&victim, &tmp).unwrap();
        assert!(write_new_owned(&path, &tmp, b"x", 0o644, None).is_err());
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "keep me");
        assert!(!path.exists());
        std::fs::remove_file(&tmp).unwrap();
        std::os::unix::fs::symlink(&victim, &path).unwrap();
        assert!(write_new_owned(&path, &tmp, b"x", 0o644, None).is_err());
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "keep me");
        assert!(std::fs::symlink_metadata(&path).unwrap().is_symlink());
        // a directory at the temp path is an error, not removed
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&tmp).unwrap();
        assert!(write_new_owned(&path, &tmp, b"x", 0o644, None).is_err());
        assert!(tmp.is_dir());
        let _ = std::fs::remove_dir_all(&root);
    }
}
