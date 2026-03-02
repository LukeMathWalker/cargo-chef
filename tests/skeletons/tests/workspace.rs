use super::*;

#[test]
pub fn ignore_vendored_directory() {
    // Arrange
    let project = CargoWorkspace::new()
        .bin_package(
            ".",
            r#"
[package]
name = "test-dummy"
version = "1.2.3"
edition = "2018"

[dependencies]
rocket = "0.5.0-rc.1"
    "#,
        )
        .file(
            ".cargo/config.toml",
            r#"
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
"#,
        )
        .lib_package(
            "vendor/rocket",
            r#"
[package]
edition = "2018"
name = "rocket"
version = "0.5.0-rc.1"
authors = ["Sergio Benitez <sb@sergio.bz>"]
build = "build.rs"
description = "Web framework with a focus on usability, security, extensibility, and speed.\n"
homepage = "https://rocket.rs"
documentation = "https://api.rocket.rs/v0.5-rc/rocket/"
readme = "../../README.md"
keywords = ["rocket", "web", "framework", "server"]
categories = ["web-programming::http-server"]
license = "MIT OR Apache-2.0"
repository = "https://github.com/SergioBenitez/Rocket"

[package.metadata.docs.rs]
all-features = true

[dependencies.rocket_dep]
version = "0.3.2"
"#,
        )
        .file(
            "vendor/rocket/.cargo-checksum.json",
            r#"
{"files": {}}
"#,
        )
        .lib_package(
            "vendor/rocket_dep",
            r#"
[package]
edition = "2018"
name = "rocket_dep"
version = "0.3.2"
authors = ["Test author"]
description = "sample package representing all of rocket's dependencies"
"#,
        )
        .file(
            "vendor/rocket_dep/.cargo-checksum.json",
            r#"
{"files": {}}
"#,
        )
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), None).unwrap();

    // Assert
    assert_eq!(1, skeleton.manifests.len());
}

#[test]
pub fn specify_member_in_workspace() {
    // Arrange
    let project = CargoWorkspace::new()
        .manifest(
            ".",
            r#"
[workspace]
members = [
    "backend",
    "ci",
]
    "#,
        )
        .bin_package(
            "backend",
            r#"
[package]
name = "backend"
version = "0.1.0"
edition = "2018"
    "#,
        )
        .bin_package(
            "ci",
            r#"
[package]
name = "ci"
version = "0.1.0"
edition = "2018"
    "#,
        )
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), "backend".to_string().into()).unwrap();

    // Assert:
    // - that "ci" is *still* in the list of `skeleton`'s manifests
    assert!(skeleton
        .manifests
        .iter()
        .any(|manifest| !manifest.contents.contains("ci")));

    // - that the list of members has been cut down to "backend", as expected
    let gold = r#"[workspace]
members = ["backend"]
"#;
    assert!(
        skeleton
            .manifests
            .iter()
            .find(|manifest| manifest.relative_path == std::path::Path::new("Cargo.toml"))
            .unwrap()
            .contents
            == gold
    );
}

#[test]
pub fn mask_workspace_dependencies() {
    // Arrange
    let project = CargoWorkspace::new()
        .manifest(
            ".",
            r#"
[workspace]
members = [
    "project_a",
    "project_b",
]

[workspace.package]
version = "0.2.0"
edition = "2021"
license = "Apache-2.0"

[workspace.dependencies]
anyhow = "1.0.66"
project_a = { path = "project_a", version = "0.2.0" }
    "#,
        )
        .bin_package(
            "project_a",
            r#"
[package]
name = "project_a"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
anyhow = { workspace = true }
    "#,
        )
        .lib_package(
            "project_b",
            r#"
[package]
name = "project_b"
version.workspace = true
edition.workspace = true
license.workspace = true

[lib]
crate-type = ["cdylib"]

[dependencies]
project_a = { workspace = true }
anyhow = { workspace = true }
    "#,
        )
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false)
        .unwrap();

    let first = skeleton.manifests[0].clone();
    check(
        &first.contents,
        expect_test::expect![[r#"
            [workspace]
            members = ["project_a", "project_b"]

            [workspace.dependencies]
            anyhow = "1.0.66"

            [workspace.dependencies.project_a]
            version = "0.0.1"
            path = "project_a"

            [workspace.package]
            edition = "2021"
            version = "0.0.1"
            license = "Apache-2.0"
        "#]],
    );

    let second = skeleton.manifests[1].clone();
    check(
        &second.contents,
        expect_test::expect![[r#"
            [[bin]]
            path = "src/main.rs"
            name = "project_a"
            plugin = false
            proc-macro = false
            required-features = []

            [package]
            name = "project_a"

            [package.edition]
            workspace = true

            [package.version]
            workspace = true

            [package.license]
            workspace = true

            [dependencies.anyhow]
            workspace = true
        "#]],
    );

    let third = skeleton.manifests[2].clone();
    check(
        &third.contents,
        expect_test::expect![[r#"
            [package]
            name = "project_b"

            [package.edition]
            workspace = true

            [package.version]
            workspace = true

            [package.license]
            workspace = true

            [dependencies.anyhow]
            workspace = true

            [dependencies.project_a]
            workspace = true

            [lib]
            path = "src/lib.rs"
            name = "project_b"
            plugin = false
            proc-macro = false
            required-features = []
            crate-type = ["cdylib"]
        "#]],
    );
}

#[test]
pub fn workspace_glob_members() {
    // Arrange
    let project = CargoWorkspace::new()
        .manifest(
            ".",
            r#"
[workspace]
members = ["crates/*"]
    "#,
        )
        .bin_package(
            "crates/project_a",
            r#"
[package]
name = "project_a"
version = "0.0.1"
    "#,
        )
        .lib_package(
            "crates/project_b",
            r#"
[package]
name = "project_b"
version = "0.0.1"
    "#,
        )
        .lib_package(
            "crates-unused/project_c",
            r#"
[package]
name = "project_c"
version = "0.0.1"
    "#,
        )
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), None).unwrap();

    // Assert
    assert_eq!(skeleton.manifests.len(), 3);
}

#[test]
pub fn filter_workspace_excludes_unrelated_member() {
    // Arrange
    let project = CargoWorkspace::new()
        .manifest(
            ".",
            r#"
[workspace]
members = ["bin_a", "bin_b"]
    "#,
        )
        .bin_package(
            "bin_a",
            r#"
[package]
name = "bin_a"
version = "0.1.0"
edition = "2021"
    "#,
        )
        .bin_package(
            "bin_b",
            r#"
[package]
name = "bin_b"
version = "0.1.0"
edition = "2021"
    "#,
        )
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), Some("bin_a".to_string())).unwrap();

    // Assert — only root workspace manifest + bin_a
    assert_eq!(skeleton.manifests.len(), 2);

    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false)
        .unwrap();
    assert!(cook_directory.path().join("bin_a/src/main.rs").exists());
    assert!(!cook_directory.path().join("bin_b/src/main.rs").exists());
}

#[test]
pub fn filter_workspace_includes_local_dependency() {
    // Arrange
    let project = CargoWorkspace::new()
        .manifest(
            ".",
            r#"
[workspace]
members = ["bin_a", "lib_a", "bin_b"]
    "#,
        )
        .bin_package(
            "bin_a",
            r#"
[package]
name = "bin_a"
version = "0.1.0"
edition = "2021"

[dependencies]
lib_a = { path = "../lib_a" }
    "#,
        )
        .lib_package(
            "lib_a",
            r#"
[package]
name = "lib_a"
version = "0.1.0"
edition = "2021"
    "#,
        )
        .bin_package(
            "bin_b",
            r#"
[package]
name = "bin_b"
version = "0.1.0"
edition = "2021"
    "#,
        )
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), Some("bin_a".to_string())).unwrap();

    // Assert — root workspace + bin_a + lib_a (bin_b excluded)
    assert_eq!(skeleton.manifests.len(), 3);

    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false)
        .unwrap();
    assert!(cook_directory.path().join("bin_a/src/main.rs").exists());
    assert!(cook_directory.path().join("lib_a/src/lib.rs").exists());
    assert!(!cook_directory.path().join("bin_b/src/main.rs").exists());
}

#[test]
pub fn filter_workspace_deps_handles_renamed_packages() {
    let project = CargoWorkspace::new()
        .manifest(
            ".",
            r#"
[workspace]
members = ["bin_a", "bin_b"]

[workspace.dependencies]
my_itoa = { package = "itoa", version = "1" }
my_ryu = { package = "ryu", version = "1" }
        "#,
        )
        .bin_package(
            "bin_a",
            r#"
[package]
name = "bin_a"
version = "0.1.0"
edition = "2021"

[dependencies]
my_itoa = { workspace = true }
        "#,
        )
        .bin_package(
            "bin_b",
            r#"
[package]
name = "bin_b"
version = "0.1.0"
edition = "2021"

[dependencies]
my_ryu = { workspace = true }
        "#,
        )
        .build();

    let skeleton = Skeleton::derive(project.path(), Some("bin_a".to_string())).unwrap();

    // Root manifest should keep my_itoa but not my_ryu
    let root = skeleton
        .manifests
        .iter()
        .find(|m| m.relative_path == std::path::Path::new("Cargo.toml"))
        .unwrap();
    assert!(root.contents.contains("my_itoa"));
    assert!(!root.contents.contains("my_ryu"));
}

#[test]
pub fn renamed_local_dependencies() {
    // Arrange
    let project = CargoWorkspace::new()
        .manifest(
            ".",
            r#"
[workspace]
members = ["a", "b"]
    "#,
        )
        .lib_package(
            "a",
            r#"
[package]
name = "a"
version = "0.5.0"

[dependencies.c]
version = "0.2.1"
package = "b"
path = "../b"
    "#,
        )
        .lib_package(
            "b",
            r#"
[package]
name = "b"
version = "0.2.1"
    "#,
        )
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), None).unwrap();

    check(
        &skeleton.manifests[1].contents,
        expect![[r#"
            [package]
            name = "a"
            version = "0.0.1"

            [dependencies.c]
            version = "0.0.1"
            path = "../b"
            package = "b"

            [lib]
            path = "src/lib.rs"
            name = "a"
            plugin = false
            proc-macro = false
            required-features = []
            crate-type = ["lib"]
        "#]],
    );
}
