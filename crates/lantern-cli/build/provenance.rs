use std::{path::Path, process::Command};

#[derive(Default)]
pub struct BuildIdentity {
    pub commit: Option<String>,
    pub dirty: Option<bool>,
}

pub fn identify(manifest: &Path) -> BuildIdentity {
    identify_checkout(manifest).unwrap_or_default()
}

fn identify_checkout(manifest: &Path) -> Option<BuildIdentity> {
    // The owning workspace must itself carry Git metadata. In particular an
    // archive nested inside another repository must not inherit its parent's SHA.
    let root = manifest.parent()?.parent()?.canonicalize().ok()?;
    if !root.join(".git").exists() {
        return None;
    }
    let toplevel = git(&root, &["rev-parse", "--show-toplevel"])?;
    if Path::new(toplevel.trim()).canonicalize().ok()? != root {
        return None;
    }
    // Do not claim an unrelated repository created around an exported source tree.
    for source in [
        "Cargo.toml",
        "crates/lantern-cli/Cargo.toml",
        "crates/lantern-cli/src/main.rs",
    ] {
        git(&root, &["cat-file", "-e", &format!("HEAD:{source}")])?;
    }
    let commit = git(&root, &["rev-parse", "--verify", "HEAD^{commit}"])?;
    let commit = commit.trim();
    if !matches!(commit.len(), 40 | 64) || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let status = git(
        &root,
        &["status", "--porcelain", "--untracked-files=normal"],
    )?;
    Some(BuildIdentity {
        commit: Some(commit.to_owned()),
        dirty: Some(!status.trim().is_empty()),
    })
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(root).args(args);
    // Caller-supplied Git routing/configuration must not redirect provenance to
    // another checkout. No external commit environment variable is trusted.
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    let output = command.output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8(output.stdout).ok())
        .flatten()
}
