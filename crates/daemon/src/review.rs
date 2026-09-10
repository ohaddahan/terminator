//! Immutable Git snapshots rendered by the pinned, app-owned CodeDiff runtime.
use anyhow::{Context, Result, bail, ensure};
use fs2::FileExt;
use std::{
    fs,
    hash::{Hash, Hasher},
    io::Read,
    os::unix::{ffi::OsStrExt, fs::PermissionsExt},
    path::{Component, Path, PathBuf},
    process::Command,
    time::Duration,
};
use terminator_core::*;
include!(concat!(env!("OUT_DIR"), "/review_assets.rs"));
const LIMIT: usize = 1024 * 1024;
/// Own snapshot lifetime even if PTY creation or process spawning fails.
pub struct Files(PathBuf);
impl Files {
    pub fn new(paths: &Paths, sid: &str) -> Self {
        Self(paths.runtime.join(format!("review-{sid}")))
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn git(root: &Path, args: &[&std::ffi::OsStr]) -> Result<Vec<u8>> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0");
    Ok(run_command(cmd, CommandOptions::default())?.stdout)
}
fn blob(root: &Path, oid: &str) -> Result<Vec<u8>> {
    if oid.bytes().all(|b| b == b'0') {
        return Ok(vec![]);
    }
    git(root, &["cat-file".as_ref(), "blob".as_ref(), oid.as_ref()])
}
fn worktree_file(path: &Path) -> Result<Vec<u8>> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e.into()),
    };
    ensure!(
        meta.is_file(),
        "Diff review supports regular files, not symlinks or submodules"
    );
    ensure!(meta.len() <= LIMIT as u64, "Diff file exceeds 1 MiB");
    let mut bytes = vec![];
    fs::File::open(path)?
        .take((LIMIT + 1) as u64)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= LIMIT, "Diff file exceeds 1 MiB");
    Ok(bytes)
}
/// Parse NUL-delimited raw diff records, retaining both paths of renames.
fn changed_blob(root: &Path, path: &Path, staged: bool) -> Result<Option<(Vec<u8>, Vec<u8>)>> {
    let mut args: Vec<&std::ffi::OsStr> = vec![
        "diff".as_ref(),
        "--raw".as_ref(),
        "-z".as_ref(),
        "--no-abbrev".as_ref(),
        "--no-ext-diff".as_ref(),
        "--no-textconv".as_ref(),
        "--find-renames".as_ref(),
    ];
    if staged {
        args.push("--cached".as_ref());
    }
    let raw = git(root, &args)?;
    let mut fields = raw.split(|b| *b == 0).filter(|f| !f.is_empty());
    while let Some(header) = fields.next() {
        let header = std::str::from_utf8(header)?;
        let parts: Vec<_> = header.split_whitespace().collect();
        ensure!(parts.len() == 5, "Invalid Git diff record");
        let first = fields.next().context("Missing Git path")?;
        let target = if parts[4].starts_with(['R', 'C']) {
            fields.next().context("Missing rename target")?
        } else {
            first
        };
        if target != path.as_os_str().as_bytes() {
            continue;
        }
        ensure!(
            !parts[4].starts_with('U'),
            "Resolve this file's merge conflict in your editor before opening a two-way diff"
        );
        ensure!(
            parts[0] != ":160000"
                && parts[1] != "160000"
                && parts[0] != ":120000"
                && parts[1] != "120000",
            "Diff review supports regular files, not symlinks or submodules"
        );
        let left = blob(root, parts[2])?;
        let right = if staged {
            blob(root, parts[3])?
        } else {
            worktree_file(&root.join(path))?
        };
        return Ok(Some((left, right)));
    }
    Ok(None)
}
pub fn snapshots(root: &Path, path: &Path, staged: bool) -> Result<(Vec<u8>, Vec<u8>)> {
    ensure!(
        !path.is_absolute() && path.components().all(|c| matches!(c, Component::Normal(_))),
        "Review path must be inside the repository"
    );
    let pair = match changed_blob(root, path, staged)? {
        Some(pair) => pair,
        None if staged => bail!("This file has no staged changes; refresh Git status"),
        None => {
            let tracked = git(
                root,
                &[
                    "ls-files".as_ref(),
                    "-z".as_ref(),
                    "--".as_ref(),
                    path.as_os_str(),
                ],
            )?;
            ensure!(
                tracked.is_empty(),
                "This file has no working-tree changes; refresh Git status"
            );
            ensure!(
                root.join(path).exists(),
                "File no longer exists; refresh Git status"
            );
            (vec![], worktree_file(&root.join(path))?)
        }
    };
    for bytes in [&pair.0, &pair.1] {
        ensure!(
            !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok(),
            "Binary or non-UTF-8 files cannot be reviewed in CodeDiff"
        );
    }
    Ok(pair)
}
fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}
fn runtime(paths: &Paths) -> Result<PathBuf> {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    ASSETS.hash(&mut hash);
    let base = paths.data.join("review-runtime");
    private_dir(&base)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(base.join("lock"))?;
    lock.lock_exclusive()?;
    let root = base.join(format!("{:016x}", hash.finish()));
    if !root.join("ready").exists() {
        private_dir(&root)?;
        for (name, bytes) in ASSETS {
            let dest = root.join(name);
            fs::create_dir_all(dest.parent().unwrap())?;
            atomic_write(&dest, bytes)?;
        }
        atomic_write(
            &root.join("ready"),
            b"61521381445aab22d5f988e7d99c2d7a76ccc1a1\n",
        )?;
    }
    Ok(root)
}
pub fn prepare(
    paths: &Paths,
    sid: &str,
    cwd: &Path,
    file: &Path,
    staged: bool,
) -> Result<portable_pty::CommandBuilder> {
    let nvim = find_executable("nvim").context(
        "Git review requires Neovim 0.10 or newer on PATH; the CodeDiff plugin is bundled",
    )?;
    let mut probe = Command::new(&nvim);
    probe.arg("--version");
    let version = bounded_output(probe, Duration::from_secs(2))?;
    let version = String::from_utf8_lossy(&version.stdout);
    let number = version
        .lines()
        .next()
        .unwrap_or("")
        .trim_start_matches("NVIM v");
    let mut parts = number.split('.');
    let major = parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    let minor = parts
        .next()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    ensure!(
        major > 0 || minor >= 10,
        "Git review requires Neovim 0.10 or newer"
    );
    let root = git(cwd, &["rev-parse".as_ref(), "--show-toplevel".as_ref()])?;
    let root = PathBuf::from(std::str::from_utf8(&root)?.trim_end_matches('\n')).canonicalize()?;
    let absolute = if file.is_absolute() {
        file.to_path_buf()
    } else {
        cwd.join(file)
    };
    let relative = absolute
        .strip_prefix(&root)
        .context("Review file is outside repository")?;
    let (left, right) = snapshots(&root, relative, staged)?;
    let runtime = runtime(paths)?;
    let dir = paths.runtime.join(format!("review-{sid}"));
    private_dir(&dir)?;
    let result = (|| {
        let filename = relative.file_name().context("Missing file name")?;
        let old = dir
            .join(if staged { "HEAD" } else { "INDEX" })
            .join(filename);
        let new = dir
            .join(if staged { "INDEX" } else { "WORKTREE" })
            .join(filename);
        for (path, bytes) in [(&old, &left), (&new, &right)] {
            private_dir(path.parent().unwrap())?;
            atomic_write(path, bytes)?;
        }
        let config = serde_json::json!({"runtime":runtime,"left":old,"right":new, "left_label":if staged { "HEAD" } else { "Index" }, "right_label":if staged { "Index" } else { "Working tree" }});
        atomic_write(&dir.join("config.json"), &serde_json::to_vec(&config)?)?;
        atomic_write(&dir.join("init.lua"), include_bytes!("review.lua"))?;
        let mut cmd = portable_pty::CommandBuilder::new(nvim);
        cmd.args(["-u"]);
        cmd.arg(dir.join("init.lua"));
        cmd.args(["-i", "NONE", "--noplugin", "-n", "--listen"]);
        cmd.arg(paths.editor_socket(sid));
        cmd.env("TERMINATOR_REVIEW_CONFIG", dir.join("config.json"));
        cmd.env("VSCODE_DIFF_NO_AUTO_INSTALL", "1");
        // Matching argv lets CodeDiff remove Neovim's redundant initial tab.
        cmd.arg("--");
        cmd.arg(old);
        cmd.arg(new);
        Ok(cmd)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&dir);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn command(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        command(dir.path(), &["init", "-q"]);
        command(
            dir.path(),
            &["config", "user.email", "fixture@example.invalid"],
        );
        command(dir.path(), &["config", "user.name", "Fixture"]);
        dir
    }
    #[test]
    fn staged_and_worktree_snapshots_preserve_partial_staging_and_renames() {
        let dir = repo();
        let root = dir.path();
        let name = Path::new("space | ' 日本.rs");
        fs::write(root.join(name), b"base\n").unwrap();
        command(root, &["add", "."]);
        command(root, &["commit", "-qm", "base"]);
        fs::write(root.join(name), b"index\n").unwrap();
        command(root, &["add", "."]);
        fs::write(root.join(name), b"working\n").unwrap();
        assert_eq!(
            snapshots(root, name, true).unwrap(),
            (b"base\n".to_vec(), b"index\n".to_vec())
        );
        assert_eq!(
            snapshots(root, name, false).unwrap(),
            (b"index\n".to_vec(), b"working\n".to_vec())
        );
        command(root, &["commit", "-qm", "index"]);
        fs::write(root.join(name), b"index\n").unwrap();
        command(root, &["mv", name.to_str().unwrap(), "renamed.rs"]);
        assert_eq!(
            snapshots(root, Path::new("renamed.rs"), true).unwrap(),
            (b"index\n".to_vec(), b"index\n".to_vec())
        );
        fs::remove_file(root.join("renamed.rs")).unwrap();
        assert_eq!(
            snapshots(root, Path::new("renamed.rs"), false).unwrap().1,
            b""
        );
    }
    #[test]
    fn linked_worktrees_use_their_own_index_and_conflicts_are_explicit() {
        let dir = repo();
        let root = dir.path();
        fs::write(root.join("file"), b"base\n").unwrap();
        command(root, &["add", "."]);
        command(root, &["commit", "-qm", "base"]);
        let branch = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["branch", "--show-current"])
            .output()
            .unwrap()
            .stdout;
        let branch = std::str::from_utf8(&branch).unwrap().trim();
        let linked = tempfile::tempdir().unwrap();
        command(
            root,
            &[
                "worktree",
                "add",
                "--detach",
                linked.path().to_str().unwrap(),
                "HEAD",
            ],
        );
        fs::write(linked.path().join("file"), b"linked\n").unwrap();
        command(linked.path(), &["add", "."]);
        assert_eq!(
            snapshots(linked.path(), Path::new("file"), true).unwrap(),
            (b"base\n".to_vec(), b"linked\n".to_vec())
        );
        assert!(snapshots(root, Path::new("file"), true).is_err());
        command(root, &["checkout", "-qb", "other"]);
        fs::write(root.join("file"), b"other\n").unwrap();
        command(root, &["commit", "-qam", "other"]);
        command(root, &["checkout", branch]);
        fs::write(root.join("file"), b"ours\n").unwrap();
        command(root, &["commit", "-qam", "ours"]);
        let merge = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["merge", "other"])
            .output()
            .unwrap();
        assert!(!merge.status.success());
        assert!(
            snapshots(root, Path::new("file"), false)
                .unwrap_err()
                .to_string()
                .contains("merge conflict")
        );
    }
    #[test]
    fn unborn_untracked_binary_and_limits_are_explicit() {
        let dir = repo();
        let root = dir.path();
        let name = Path::new("new.txt");
        fs::write(root.join(name), b"new\n").unwrap();
        assert_eq!(
            snapshots(root, name, false).unwrap(),
            (vec![], b"new\n".to_vec())
        );
        command(root, &["add", "."]);
        assert_eq!(
            snapshots(root, name, true).unwrap(),
            (vec![], b"new\n".to_vec())
        );
        fs::write(root.join(name), b"binary\0").unwrap();
        assert!(
            snapshots(root, name, false)
                .unwrap_err()
                .to_string()
                .contains("Binary")
        );
        fs::write(root.join(name), vec![b'a'; LIMIT + 1]).unwrap();
        assert!(
            snapshots(root, name, false)
                .unwrap_err()
                .to_string()
                .contains("1 MiB")
        );
        assert!(snapshots(root, Path::new("../outside"), false).is_err());
    }
}
