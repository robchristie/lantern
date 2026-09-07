#[path = "build/provenance.rs"]
mod provenance;

fn main() {
    let manifest = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    // Git refs, packed refs, detached HEADs, linked worktrees and index/worktree
    // state can change without a source edit. Deliberately refresh on every Cargo
    // invocation; this absent output path is never created by the build script.
    let refresh = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap())
        .join("refresh-build-provenance");
    println!("cargo:rerun-if-changed={}", refresh.display());
    let identity = provenance::identify(&manifest);
    println!(
        "cargo:rustc-env=LANTERN_BUILD_COMMIT={}",
        identity.commit.as_deref().unwrap_or("")
    );
    println!(
        "cargo:rustc-env=LANTERN_BUILD_PROVENANCE={}",
        if identity.commit.is_some() {
            "git"
        } else {
            "unknown"
        }
    );
    println!(
        "cargo:rustc-env=LANTERN_BUILD_DIRTY={}",
        match identity.dirty {
            Some(true) => "true",
            Some(false) => "false",
            None => "unknown",
        }
    );
}
