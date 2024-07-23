//! Details for git repository

use axoprocess::Cmd;
use camino::{Utf8Path, Utf8PathBuf};

use crate::errors::Result;

/// Information about a git repo
#[derive(Clone, Debug)]
pub struct LocalRepo {
    /// The repository's absolute path on disk
    pub path: Utf8PathBuf,
    /// The repository's current HEAD
    /// This can be None in the case that a git repository
    /// has been `init`ted, but no commits have been made yet.
    pub head: Option<String>,
}

impl LocalRepo {
    /// Returns a Repo for the git repository at `working_dir`.
    /// If git returns an error, for example if `working_dir`
    /// isn't a git repository, this will return an `Err`.
    /// The `git` param is the path to the `git` executable to use.
    pub fn new(git: &str, working_dir: &Utf8Path) -> Result<Self> {
        let root = get_root(git, working_dir)?;
        let path = Utf8PathBuf::from(root);
        let head = get_head_commit(git, working_dir).ok();

        Ok(Self { path, head })
    }
}

fn get_root(git: &str, working_dir: &Utf8Path) -> Result<String> {
    let mut cmd = Cmd::new(git, "detect a git repo");
    cmd.arg("rev-parse")
        .arg("--show-toplevel")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .check(false)
        .current_dir(working_dir);

    let result = cmd.output()?;

    let root = String::from_utf8(result.stdout)?;
    Ok(root.trim_end().to_owned())
}

fn get_head_commit(git: &str, working_dir: &Utf8Path) -> Result<String> {
    let mut cmd = Cmd::new(git, "check for HEAD commit");
    cmd.arg("rev-parse")
        .arg("HEAD")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .check(false)
        .current_dir(working_dir);

    let result = cmd.output()?;

    let commit = String::from_utf8(result.stdout)?;
    Ok(commit.trim_end().to_owned())
}
