//! Exercise real Cargo invalidation with tiny offline source workspaces. These
//! probes use the shipped build script, avoiding a duplicate provenance model.
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "lantern-provenance-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn checked(command: &mut Command) -> String {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "command failed: {command:?}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}
fn git(root: &Path, args: &[&str]) -> String {
    let mut command = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    checked(
        command
            .arg("-C")
            .arg(root)
            .arg("-c")
            .arg("user.name=Lantern fixture")
            .arg("-c")
            .arg("user.email=fixture@example.invalid")
            .arg("-c")
            .arg("commit.gpgsign=false")
            .args(args),
    )
}
fn source(root: &Path) {
    let cli = root.join("crates/lantern-cli");
    fs::create_dir_all(cli.join("src")).unwrap();
    fs::create_dir_all(cli.join("build")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers=[\"crates/lantern-cli\"]\nresolver=\"2\"\n",
    )
    .unwrap();
    fs::write(root.join(".gitignore"), "target/\n").unwrap();
    fs::write(
        cli.join("Cargo.toml"),
        "[package]\nname=\"provenance-probe\"\nversion=\"0.0.0\"\nedition=\"2021\"\n",
    )
    .unwrap();
    fs::write(cli.join("src/main.rs"), r#"fn main() { println!("{}|{}|{}", env!("LANTERN_BUILD_PROVENANCE"), env!("LANTERN_BUILD_COMMIT"), env!("LANTERN_BUILD_DIRTY")); }"#).unwrap();
    fs::write(cli.join("build.rs"), include_str!("../build.rs")).unwrap();
    fs::write(
        cli.join("build/provenance.rs"),
        include_str!("../build/provenance.rs"),
    )
    .unwrap();
}
fn build(root: &Path, target: &Path, redirect: Option<&Path>) -> String {
    let mut cargo = Command::new(env!("CARGO"));
    cargo
        .current_dir(root)
        .args(["build", "--offline", "--quiet"])
        .arg("--target-dir")
        .arg(target)
        .env("RUSTC_WRAPPER", "")
        .env("LANTERN_BUILD_COMMIT", "untrusted-build-override");
    if let Some(redirect) = redirect {
        cargo
            .env("GIT_DIR", redirect.join(".git"))
            .env("GIT_WORK_TREE", redirect);
    }
    checked(&mut cargo);
    checked(&mut Command::new(target.join("debug/provenance-probe")))
}

#[test]
fn cargo_refreshes_commit_and_modified_state_for_refs_and_linked_worktrees() {
    let scratch = Scratch::new();
    let root = scratch.0.join("checkout");
    source(&root);
    git(&root, &["init", "--quiet"]);
    // Generate the lockfile before committing a clean fixture.
    checked(Command::new(env!("CARGO")).current_dir(&root).args([
        "generate-lockfile",
        "--offline",
        "--quiet",
    ]));
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "Create fixture"]);
    let first = git(&root, &["rev-parse", "HEAD"]);
    let target = scratch.0.join("target");
    assert_eq!(build(&root, &target, None), format!("git|{first}|false"));
    git(
        &root,
        &[
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "Advance ref without source changes",
        ],
    );
    let second = git(&root, &["rev-parse", "HEAD"]);
    assert_ne!(first, second);
    assert_eq!(build(&root, &target, None), format!("git|{second}|false"));
    git(&root, &["pack-refs", "--all"]);
    git(&root, &["checkout", "--quiet", "--detach", &first]);
    assert_eq!(build(&root, &target, None), format!("git|{first}|false"));
    fs::write(root.join("untracked-source.txt"), "new source").unwrap();
    assert_eq!(build(&root, &target, None), format!("git|{first}|true"));
    fs::remove_file(root.join("untracked-source.txt")).unwrap();
    fs::write(root.join(".gitignore"), "target/\n# tracked modification\n").unwrap();
    assert_eq!(build(&root, &target, None), format!("git|{first}|true"));
    git(&root, &["checkout", "--", ".gitignore"]);
    assert_eq!(build(&root, &target, None), format!("git|{first}|false"));
    let linked = scratch.0.join("linked");
    git(
        &root,
        &[
            "worktree",
            "add",
            "--quiet",
            "--detach",
            linked.to_str().unwrap(),
            &second,
        ],
    );
    assert_eq!(
        build(&linked, &target, Some(&root)),
        format!("git|{second}|false")
    );
    git(&root, &["worktree", "remove", linked.to_str().unwrap()]);
}

#[test]
fn exported_sources_never_inherit_an_unrelated_parent_or_git_environment() {
    let scratch = Scratch::new();
    let unrelated = scratch.0.join("unrelated");
    source(&unrelated);
    git(&unrelated, &["init", "--quiet"]);
    git(&unrelated, &["add", "."]);
    git(
        &unrelated,
        &["commit", "--quiet", "-m", "Create unrelated repository"],
    );
    let archive = unrelated.join("exported-source");
    source(&archive);
    let target = scratch.0.join("target");
    assert_eq!(
        build(&archive, &target, Some(&unrelated)),
        "unknown||unknown"
    );
    // Even a local .git does not confer identity when Lantern is untracked.
    git(&archive, &["init", "--quiet"]);
    git(
        &archive,
        &["commit", "--quiet", "--allow-empty", "-m", "Unrelated root"],
    );
    assert_eq!(
        build(&archive, &target, Some(&unrelated)),
        "unknown||unknown"
    );
    git(&archive, &["add", "."]);
    assert_eq!(
        build(&archive, &target, Some(&unrelated)),
        "unknown||unknown"
    );
}
