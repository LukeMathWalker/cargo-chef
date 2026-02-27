use super::*;

#[test]
pub fn rust_toolchain() {
    // Arrange
    let project = CargoWorkspace::new()
        .manifest(
            ".",
            r#"
[package]
name = "test-dummy"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
        )
        .touch("src/main.rs")
        .touch("Cargo.lock")
        .file("rust-toolchain", "1.75.0")
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false)
        .unwrap();

    // Assert
    assert_eq!(1, skeleton.manifests.len());
    let manifest = &skeleton.manifests[0];
    assert_eq!(Path::new("Cargo.toml"), manifest.relative_path);
    cook_directory
        .child("src")
        .child("main.rs")
        .assert("fn main() {}");
    cook_directory
        .child("Cargo.lock")
        .assert(predicate::path::exists());
    cook_directory.child("rust-toolchain").assert("1.75.0");
}

#[test]
pub fn rust_toolchain_toml() {
    // Arrange
    let project = CargoWorkspace::new()
        .manifest(
            ".",
            r#"
[package]
name = "test-dummy"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
        )
        .touch("src/main.rs")
        .touch("Cargo.lock")
        .file(
            "rust-toolchain.toml",
            r#"
[toolchain]
channel = "1.75.0"
"#,
        )
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false)
        .unwrap();

    // Assert
    assert_eq!(1, skeleton.manifests.len());
    let manifest = &skeleton.manifests[0];
    assert_eq!(Path::new("Cargo.toml"), manifest.relative_path);
    cook_directory
        .child("src")
        .child("main.rs")
        .assert("fn main() {}");
    cook_directory
        .child("Cargo.lock")
        .assert(predicate::path::exists());
    cook_directory.child("rust-toolchain.toml").assert(
        r#"[toolchain]
channel = "1.75.0"
"#,
    );
}
/// See https://github.com/LukeMathWalker/cargo-chef/issues/232.
#[test]
pub fn workspace_bin_nonstandard_dirs() {
    // Arrange
    let project = CargoWorkspace::new()
        .manifest(
            ".",
            r#"
[workspace]
members = [
    "crates/client/project_a",
    "crates/client/project_b",
    "crates/server/*",
    "vendored/project_e",
    "project_f",
]        
"#,
        )
        .bin_package(
            "crates/client/project_a",
            r#"
[package]
name = "project_a"
version = "0.1.0"
edition = "2018"

[dependencies]
uuid = { version = "=0.8.0", features = ["v4"] }
"#,
        )
        .bin_package(
            "crates/client/project_b",
            r#"
[package]
name = "project_b"
version = "0.1.0"
edition = "2018"

[dependencies]
uuid = { version = "=0.8.0", features = ["v4"] }
"#,
        )
        .bin_package(
            "crates/server/project_c",
            r#"
[package]
name = "project_c"
version = "0.1.0"
edition = "2018"

[dependencies]
uuid = { version = "=0.8.0", features = ["v4"] }
"#,
        )
        .bin_package(
            "crates/server/project_d",
            r#"
[package]
name = "project_d"
version = "0.1.0"
edition = "2018"

[dependencies]
uuid = { version = "=0.8.0", features = ["v4"] }
"#,
        )
        .bin_package(
            "vendored/project_e",
            r#"
[package]
name = "project_e"
version = "0.1.0"
edition = "2018"

[dependencies]
uuid = { version = "=0.8.0", features = ["v4"] }
"#,
        )
        .bin_package(
            "project_f",
            r#"
[package]
name = "project_f"
version = "0.1.0"
edition = "2018"

[dependencies]
uuid = { version = "=0.8.0", features = ["v4"] }
"#,
        )
        .build();

    fn manifest_content_dirs(skeleton: &Skeleton) -> Vec<String> {
        // This is really ugly... sorry.
        skeleton
            .manifests
            .first()
            .unwrap()
            .contents
            .split('=')
            .next_back()
            .unwrap()
            .replace(['[', ']', '"'], "")
            .trim()
            .split(',')
            .map(|w| w.trim().to_string())
            .collect()
    }

    // Act
    let path = project.path();
    let all = Skeleton::derive(&path, None).unwrap();
    assert_eq!(
        manifest_content_dirs(&all),
        vec![
            "crates/client/project_a",
            "crates/client/project_b",
            "crates/server/*",
            "vendored/project_e",
            "project_f"
        ]
    );

    let project_a = Skeleton::derive(&path, Some("project_a".into())).unwrap();
    assert_eq!(
        manifest_content_dirs(&project_a),
        vec!["crates/client/project_a"]
    );

    let project_b = Skeleton::derive(&path, Some("project_b".into())).unwrap();
    assert_eq!(
        manifest_content_dirs(&project_b),
        vec!["crates/client/project_b"]
    );

    let project_c = Skeleton::derive(&path, Some("project_c".into())).unwrap();
    assert_eq!(
        manifest_content_dirs(&project_c),
        vec!["crates/server/project_c"]
    );

    let project_d = Skeleton::derive(&path, Some("project_d".into())).unwrap();
    assert_eq!(
        manifest_content_dirs(&project_d),
        vec!["crates/server/project_d"]
    );

    let project_e = Skeleton::derive(&path, Some("project_e".into())).unwrap();
    assert_eq!(
        manifest_content_dirs(&project_e),
        vec!["vendored/project_e"]
    );

    let project_f = Skeleton::derive(&path, Some("project_f".into())).unwrap();
    assert_eq!(manifest_content_dirs(&project_f), vec!["project_f"]);

    // TODO: If multiple binaries are valid in `cargo chef prepare`, then testing
    // with multiple binaries is probably a good idea here!
}
