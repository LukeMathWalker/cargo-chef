//! Strip all `path = "…"` dependencies from a skeleton to produce a
//! "third-party only" recipe whose content is stable across any pure
//! workspace-internal change.
//!
//! # Why
//!
//! `cargo chef prepare` captures every workspace manifest including the
//! `path = "..."` dependencies.  Any structural change to the
//! workspace (adding a crate, splitting one, renaming a binary target, etc.)
//! changes `recipe.json`, which invalidates the `cargo chef cook` Docker layer
//! and forces a full recompile of all third-party crates.
//!
//! `strip_path_deps` post-processes an already-derived [`Skeleton`] by:
//!
//! 1. Removing **every** dependency entry that carries a `path = "..."` field
//!    from every manifest in the skeleton (top-level sections, target-specific
//!    sections, and `[workspace.dependencies]`).  This includes both
//!    intra-workspace path deps and any other local path deps (e.g. out-of-tree
//!    crates referenced by absolute or relative path).
//!
//! 2. Removing all local (no-`source`) package entries from the lock file.
//!    External packages always carry a `source` field (e.g.
//!    `"registry+https://github.com/rust-lang/crates.io-index"`); local
//!    workspace members do not.  Removing them makes the recipe immune to
//!    workspace-membership changes (adding/removing internal crates).
//!
//! The resulting skeleton is then serialised into `recipe.json`.  Because it
//! no longer contains any path-dep information, the Docker layer
//! produced by `cargo chef cook` from this recipe is only invalidated when an
//! *external* dependency actually changes.

use std::collections::HashSet;

use anyhow::Context;

use super::Manifest;

const DEP_SECTIONS: &[&str] = &["dependencies", "dev-dependencies", "build-dependencies"];

/// Remove all `path = "…"` dependency entries from every manifest and all
/// local package entries from the lock file.
///
/// Uses a two-pass approach to handle workspace-inherited path deps:
///
/// 1. Scan every manifest for `[workspace.dependencies]` entries that carry
///    `path = "…"` and collect their names.
/// 2. Strip direct path dep entries **and** any `dep = { workspace = true }`
///    references whose name appears in the set collected in step 1.
///
/// This ensures that after stripping, no manifest references a workspace dep
/// that no longer exists in `[workspace.dependencies]`.
pub(super) fn strip_path_deps(
    manifests: &mut [Manifest],
    lock_file: &mut Option<String>,
) -> anyhow::Result<()> {
    // Pass 1: collect names of workspace-inherited deps that are path deps.
    let workspace_path_dep_names: HashSet<String> = manifests
        .iter()
        .flat_map(|m| collect_workspace_path_dep_names(&m.contents))
        .collect();

    // Pass 2: strip from every manifest.
    for manifest in manifests.iter_mut() {
        manifest.contents =
            strip_from_manifest_contents(&manifest.contents, &workspace_path_dep_names)
                .with_context(|| {
                    format!(
                        "failed to strip path deps from manifest {}",
                        manifest.relative_path.display()
                    )
                })?;
    }
    if let Some(lock) = lock_file {
        *lock = strip_local_packages_from_lock(lock)
            .context("failed to strip local packages from lock file")?;
    }
    Ok(())
}

/// Collect the names of all `[workspace.dependencies]` entries that have a
/// `path` field.  Returns an empty `HashSet` when the manifest has no
/// `[workspace.dependencies]` section or when it is not parseable.
fn collect_workspace_path_dep_names(contents: &str) -> HashSet<String> {
    let value: toml::Value = match toml::from_str(contents) {
        Ok(v) => v,
        Err(_) => return HashSet::new(),
    };
    value
        .get("workspace")
        .and_then(|ws| ws.get("dependencies"))
        .and_then(|d| d.as_table())
        .map(|table| {
            table
                .iter()
                .filter_map(|(name, spec)| {
                    if spec.get("path").is_some() {
                        Some(name.clone())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Remove all `path = "…"` entries from a single manifest's TOML contents,
/// and remove any `{ workspace = true }` references whose dep name is in
/// `workspace_path_dep_names`.
///
/// Handles:
/// - `[dependencies]` / `[dev-dependencies]` / `[build-dependencies]`
/// - `[target.'cfg(...)'.dependencies]` (and dev/build variants)
/// - `[workspace.dependencies]`
fn strip_from_manifest_contents(
    contents: &str,
    workspace_path_dep_names: &HashSet<String>,
) -> anyhow::Result<String> {
    let mut value: toml::Value = toml::from_str(contents)
        .context("failed to parse manifest TOML during path-dep stripping")?;

    // Top-level [dependencies], [dev-dependencies], [build-dependencies]
    strip_path_deps_from_value(&mut value, workspace_path_dep_names);

    // [target.'cfg(...)'.{dependencies,dev-dependencies,build-dependencies}]
    if let Some(targets) = value.get_mut("target") {
        if let Some(targets_table) = targets.as_table_mut() {
            for (_, target_cfg) in targets_table.iter_mut() {
                strip_path_deps_from_value(target_cfg, workspace_path_dep_names);
            }
        }
    }

    // [workspace.dependencies] (workspace inheritance, rust 1.64+)
    if let Some(workspace) = value.get_mut("workspace") {
        strip_path_deps_from_value(workspace, workspace_path_dep_names);
    }

    toml::to_string(&value)
        .context("failed to re-serialise manifest TOML after stripping path deps")
}

/// Remove every dependency entry that has a `path` field, or whose name
/// appears in `workspace_path_dep_names` with `{ workspace = true }`, from
/// the given `toml::Value`'s dependency sections.
fn strip_path_deps_from_value(value: &mut toml::Value, workspace_path_dep_names: &HashSet<String>) {
    for section in DEP_SECTIONS {
        if let Some(deps) = value.get_mut(section) {
            if let Some(table) = deps.as_table_mut() {
                table.retain(|name, dep_spec| {
                    // Remove direct path deps.
                    if dep_spec.get("path").is_some() {
                        return false;
                    }
                    // Remove `{ workspace = true }` references to path deps
                    // that were declared in [workspace.dependencies] with path =
                    // and have now been stripped from that section.
                    let is_workspace_ref = dep_spec
                        .get("workspace")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if is_workspace_ref && workspace_path_dep_names.contains(name) {
                        return false;
                    }
                    true
                });
            }
        }
    }
}

/// Remove all local (no-`source`) `[[package]]` entries from a serialised
/// Cargo.lock TOML string.
fn strip_local_packages_from_lock(lock_str: &str) -> anyhow::Result<String> {
    let mut lock: toml::Value = toml::from_str(lock_str)
        .context("failed to parse lock file TOML during path-dep stripping")?;
    if let Some(packages) = lock.get_mut("package").and_then(|p| p.as_array_mut()) {
        // Keep only packages that have a `source` field — those are external.
        packages.retain(|pkg| pkg.get("source").is_some());
    }
    toml::to_string(&lock)
        .context("failed to re-serialise lock file TOML after stripping local packages")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(contents: &str) -> Manifest {
        use std::path::PathBuf;
        Manifest {
            relative_path: PathBuf::from("Cargo.toml"),
            contents: contents.to_string(),
            targets: vec![],
        }
    }

    #[test]
    fn strips_path_dep_from_dependencies() {
        let m = manifest(
            r#"
[package]
name = "app"
version = "0.1.0"

[dependencies]
anyhow = "1"

[dependencies.internal]
path = "../internal"
version = "0.0.1"
"#,
        );
        let mut manifests = vec![m];
        let mut lock: Option<String> = None;
        strip_path_deps(&mut manifests, &mut lock).expect("strip_path_deps failed");

        let contents = &manifests[0].contents;
        assert!(
            !contents.contains("path ="),
            "path dep should be removed:\n{}",
            contents
        );
        assert!(
            contents.contains("anyhow"),
            "external dep should be kept:\n{}",
            contents
        );
    }

    #[test]
    fn strips_out_of_workspace_path_dep() {
        // The implementation removes ANY dep with a `path` field, not just
        // intra-workspace ones.  Verify an absolute or out-of-tree relative
        // path dep is also removed.
        let m = manifest(
            r#"
[package]
name = "app"
version = "0.1.0"

[dependencies]
anyhow = "1"

[dependencies.external-local]
path = "/opt/some-other-project"
version = "0.1.0"
"#,
        );
        let mut manifests = vec![m];
        let mut lock: Option<String> = None;
        strip_path_deps(&mut manifests, &mut lock).expect("strip_path_deps failed");

        let contents = &manifests[0].contents;
        assert!(
            !contents.contains("path ="),
            "out-of-workspace path dep should be removed:\n{}",
            contents
        );
        assert!(
            contents.contains("anyhow"),
            "external dep should be kept:\n{}",
            contents
        );
    }

    #[test]
    fn keeps_external_deps_untouched() {
        let original = r#"
[package]
name = "model"
version = "0.1.0"

[dependencies.serde]
version = "1"
features = ["derive"]
"#;
        let result = strip_from_manifest_contents(original, &HashSet::new()).expect("strip failed");
        assert!(
            result.contains("serde"),
            "serde should be kept:\n{}",
            result
        );
        assert!(
            !result.contains("path ="),
            "no path dep in result:\n{}",
            result
        );
    }

    #[test]
    fn strips_path_dep_from_workspace_dependencies() {
        let original = r#"
[workspace]
members = ["a", "b"]

[workspace.dependencies]
serde = "1"

[workspace.dependencies.internal]
path = "./internal"
version = "0.0.1"
"#;
        let result = strip_from_manifest_contents(original, &HashSet::new()).expect("strip failed");
        assert!(result.contains("serde"), "serde kept:\n{}", result);
        assert!(!result.contains("path ="), "path dep removed:\n{}", result);
    }

    #[test]
    fn strips_path_dep_from_target_specific_dependencies() {
        let original = r#"
[package]
name = "platform_crate"
version = "0.1.0"

[target.'cfg(unix)'.dependencies.internal]
path = "../internal"
version = "0.0.1"

[target.'cfg(unix)'.dependencies]
libc = "0.2"
"#;
        let result = strip_from_manifest_contents(original, &HashSet::new()).expect("strip failed");
        assert!(result.contains("libc"), "libc kept:\n{}", result);
        assert!(!result.contains("path ="), "path dep removed:\n{}", result);
    }

    #[test]
    fn strips_workspace_true_refs_to_workspace_path_deps() {
        // The workspace root defines `internal = { path = "./internal" }` in
        // [workspace.dependencies].  A member crate references it via
        // `internal = { workspace = true }`.  After stripping, both the
        // workspace declaration and the member reference must be gone.
        let workspace_root = r#"
[workspace]
members = ["member"]

[workspace.dependencies]
serde = "1"

[workspace.dependencies.internal]
path = "./internal"
version = "0.0.1"
"#;
        let member = r#"
[package]
name = "member"
version = "0.1.0"

[dependencies]
serde = { workspace = true }
internal = { workspace = true }
"#;
        // The path dep names come from the workspace root.
        let ws_path_names = collect_workspace_path_dep_names(workspace_root);
        assert!(
            ws_path_names.contains("internal"),
            "should detect 'internal' as workspace path dep"
        );

        let stripped_member =
            strip_from_manifest_contents(member, &ws_path_names).expect("strip failed");
        assert!(
            stripped_member.contains("serde"),
            "serde (external) should be kept:\n{}",
            stripped_member
        );
        assert!(
            !stripped_member.contains("internal"),
            "'internal' workspace = true ref should be removed:\n{}",
            stripped_member
        );
    }

    #[test]
    fn strips_local_packages_from_lock_file() {
        let lock = r#"
version = 3

[[package]]
name = "anyhow"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "abc"

[[package]]
name = "my-local-crate"
version = "0.0.1"

[[package]]
name = "serde"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "def"
"#;
        let result = strip_local_packages_from_lock(lock).expect("strip lock failed");
        assert!(result.contains("anyhow"), "anyhow kept:\n{}", result);
        assert!(result.contains("serde"), "serde kept:\n{}", result);
        assert!(
            !result.contains("my-local-crate"),
            "local crate removed:\n{}",
            result
        );
    }

    #[test]
    fn strips_all_path_deps_leaving_valid_manifest() {
        // Edge case: every dependency in the manifest is a path dep.
        // After stripping the [dependencies] section should be empty (or
        // absent), and the resulting TOML must still be parseable.
        let m = manifest(
            r#"
[package]
name = "app"
version = "0.1.0"

[dependencies.core]
path = "../core"

[dependencies.util]
path = "../util"
"#,
        );
        let mut manifests = vec![m];
        let mut lock: Option<String> = None;
        strip_path_deps(&mut manifests, &mut lock).expect("strip_path_deps failed");

        let contents = &manifests[0].contents;
        // No path dep entries must remain.
        assert!(
            !contents.contains("path ="),
            "all path deps should be removed:\n{}",
            contents
        );
        // The result must still be valid TOML.
        toml::from_str::<toml::Value>(contents).expect("stripped manifest must be valid TOML");
    }

    #[test]
    fn returns_error_on_invalid_manifest_toml() {
        // strip_from_manifest_contents should return Err, not panic, when fed
        // malformed TOML.
        let invalid = "this is [ not valid toml !!!";
        let result = strip_from_manifest_contents(invalid, &HashSet::new());
        assert!(
            result.is_err(),
            "expected Err for invalid manifest TOML, got Ok"
        );
    }

    #[test]
    fn returns_error_on_invalid_lock_file_toml() {
        // strip_local_packages_from_lock should return Err, not panic, when fed
        // malformed TOML.
        let invalid = "this is [ not valid toml !!!";
        let result = strip_local_packages_from_lock(invalid);
        assert!(
            result.is_err(),
            "expected Err for invalid lock file TOML, got Ok"
        );
    }

    #[test]
    fn strip_path_deps_propagates_manifest_parse_error() {
        // strip_path_deps should propagate errors from invalid manifests rather
        // than panicking.
        let bad = Manifest {
            relative_path: std::path::PathBuf::from("bad/Cargo.toml"),
            contents: "this is [ not valid toml !!!".to_string(),
            targets: vec![],
        };
        let mut manifests = vec![bad];
        let mut lock: Option<String> = None;
        let result = strip_path_deps(&mut manifests, &mut lock);
        assert!(
            result.is_err(),
            "expected Err when manifest has invalid TOML, got Ok"
        );
    }
}
