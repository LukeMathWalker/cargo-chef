use super::*;

/// Verify that `--external-only` / `Skeleton::strip_path_deps` removes all
/// intra-workspace path dependencies from every manifest and all local
/// package entries from the lock file, while leaving external crates intact.

#[test]
fn external_only_strips_path_deps_from_manifests() {
    // Arrange: workspace with a binary that depends on an internal lib and an
    // external crate.
    let project = CargoWorkspace::new()
        .manifest(
            ".",
            r#"
[workspace]
members = ["app", "model"]
"#,
        )
        .bin_package(
            "app",
            r#"
[package]
name = "app"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"

[dependencies.model]
path = "../model"
version = "0.0.1"
"#,
        )
        .lib_package(
            "model",
            r#"
[package]
name = "model"
version = "0.1.0"
edition = "2021"

[dependencies.serde]
version = "1"
features = ["derive"]
"#,
        )
        .touch("Cargo.lock")
        .build();

    // Act
    let mut skeleton = Skeleton::derive(project.path(), None).unwrap();
    skeleton.strip_path_deps();

    // Assert: no manifest should contain an intra-workspace path dep.
    // We check for `path = "../` (relative paths are the hallmark of
    // workspace-internal deps). Binary/lib target paths like
    // `path = "src/main.rs"` are NOT relative upward and are kept.
    for manifest in &skeleton.manifests {
        // We cannot use a blanket `!contains("path =")` because [[bin]]/[[lib]]
        // target entries keep their `path = "src/..."` source-file pointers,
        // which is correct behaviour.  Instead check that the known intra-workspace
        // dep section is gone.
        assert!(
            !manifest.contents.contains("[dependencies.model]"),
            "intra-workspace dep [dependencies.model] found in {} after strip_path_deps:\n{}",
            manifest.relative_path.display(),
            manifest.contents,
        );
    }

    // Assert: external deps are preserved
    let app_manifest = skeleton
        .manifests
        .iter()
        .find(|m| m.relative_path.ends_with("app/Cargo.toml"))
        .expect("app/Cargo.toml should be present");
    assert!(
        app_manifest.contents.contains("anyhow"),
        "anyhow should be kept in app manifest:\n{}",
        app_manifest.contents,
    );

    let model_manifest = skeleton
        .manifests
        .iter()
        .find(|m| m.relative_path.ends_with("model/Cargo.toml"))
        .expect("model/Cargo.toml should be present");
    assert!(
        model_manifest.contents.contains("serde"),
        "serde should be kept in model manifest:\n{}",
        model_manifest.contents,
    );
}

#[test]
fn external_only_strips_local_packages_from_lock_file() {
    // Arrange: workspace with a real Cargo.lock containing both local and
    // external entries.  We write a hand-crafted lock file that mimics the
    // structure cargo-chef serialises (toml::to_string output).
    let lock_contents = r#"
version = 3

[[package]]
name = "anyhow"
version = "1.0.100"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "abc123"

[[package]]
name = "app"
version = "0.0.1"

[[package]]
name = "model"
version = "0.0.1"

[[package]]
name = "serde"
version = "1.0.200"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "def456"
dependencies = [
 "serde_derive",
]

[[package]]
name = "serde_derive"
version = "1.0.200"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "ghi789"
"#;

    let project = CargoWorkspace::new()
        .manifest(
            ".",
            r#"
[workspace]
members = ["app", "model"]
"#,
        )
        .bin_package(
            "app",
            r#"
[package]
name = "app"
version = "0.1.0"

[dependencies.model]
path = "../model"
"#,
        )
        .lib_package(
            "model",
            r#"
[package]
name = "model"
version = "0.1.0"
"#,
        )
        .file("Cargo.lock", lock_contents)
        .build();

    // Act
    let mut skeleton = Skeleton::derive(project.path(), None).unwrap();
    skeleton.strip_path_deps();

    // Assert: local workspace packages must not appear in the lock file
    let lock_file = skeleton
        .lock_file
        .as_deref()
        .expect("lock file should be present");

    assert!(
        !lock_file.contains("\"app\""),
        "local 'app' should be removed from lock:\n{}",
        lock_file,
    );
    assert!(
        !lock_file.contains("\"model\""),
        "local 'model' should be removed from lock:\n{}",
        lock_file,
    );

    // External packages must remain
    assert!(
        lock_file.contains("anyhow"),
        "anyhow should be kept in lock:\n{}",
        lock_file,
    );
    assert!(
        lock_file.contains("serde"),
        "serde should be kept in lock:\n{}",
        lock_file,
    );
}

#[test]
fn external_only_stable_across_workspace_internal_change() {
    // Verify the core property: two skeletons that differ only in their
    // workspace-internal path deps (but share the same external deps) produce
    // identical recipes after strip_path_deps.

    let base_manifest = r#"
[workspace]
members = ["app", "model"]
"#;

    // Version A: app depends on model
    let project_a = CargoWorkspace::new()
        .manifest(".", base_manifest)
        .bin_package(
            "app",
            r#"
[package]
name = "app"
version = "0.1.0"

[dependencies]
anyhow = "1"

[dependencies.model]
path = "../model"
version = "0.0.1"
"#,
        )
        .lib_package(
            "model",
            r#"
[package]
name = "model"
version = "0.1.0"

[dependencies.serde]
version = "1"
"#,
        )
        .touch("Cargo.lock")
        .build();

    // Version B: app no longer depends on model (simulates removing an
    // internal dependency — a pure workspace-internal change)
    let project_b = CargoWorkspace::new()
        .manifest(".", base_manifest)
        .bin_package(
            "app",
            r#"
[package]
name = "app"
version = "0.1.0"

[dependencies]
anyhow = "1"
"#,
        )
        .lib_package(
            "model",
            r#"
[package]
name = "model"
version = "0.1.0"

[dependencies.serde]
version = "1"
"#,
        )
        .touch("Cargo.lock")
        .build();

    // Act
    let mut skeleton_a = Skeleton::derive(project_a.path(), None).unwrap();
    skeleton_a.strip_path_deps();

    let mut skeleton_b = Skeleton::derive(project_b.path(), None).unwrap();
    skeleton_b.strip_path_deps();

    // Assert: after stripping, both skeletons are identical (same manifests)
    // Sort by relative_path first to guard against non-deterministic filesystem
    // traversal order — the content comparison must not depend on iteration order.
    skeleton_a
        .manifests
        .sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    skeleton_b
        .manifests
        .sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    assert_eq!(
        skeleton_a.manifests.len(),
        skeleton_b.manifests.len(),
        "manifest count should be equal"
    );
    for (ma, mb) in skeleton_a.manifests.iter().zip(skeleton_b.manifests.iter()) {
        assert_eq!(
            ma.contents,
            mb.contents,
            "manifest {} should be identical after strip_path_deps",
            ma.relative_path.display()
        );
    }
}
