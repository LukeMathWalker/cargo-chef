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

#[test]
pub fn filter_workspace_excludes_unrelated_member() {
    // bin_a and bin_b are independent members.
    // When targeting bin_a, bin_b should be excluded from the skeleton.
    let project = CargoWorkspace::new()
        .manifest(
            ".",
            r#"
[workspace]
members = [
    "bin_a",
    "bin_b",
]
    "#,
        )
        .bin_package(
            "bin_a",
            r#"
[package]
name = "bin_a"
version = "0.1.0"
edition = "2018"

[dependencies]
uuid = { version = "=0.8.0", features = ["v4"] }
    "#,
        )
        .bin_package(
            "bin_b",
            r#"
[package]
name = "bin_b"
version = "0.1.0"
edition = "2018"

[dependencies]
    "#,
        )
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), Some("bin_a".to_string())).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false)
        .unwrap();

    // Assert
    assert_eq!(2, skeleton.manifests.len());
    cook_directory
        .child("bin_a")
        .child("src")
        .child("main.rs")
        .assert("fn main() {}");
}

#[test]
pub fn filter_workspace_includes_local_dependency() {
    // bin_a depends on lib_a.
    // When targeting bin_a, lib_a should be included in the skeleton.
    let project = CargoWorkspace::new()
        .manifest(
            ".",
            r#"
[workspace]
members = [
    "bin_a",
    "lib_a",
]
    "#,
        )
        .bin_package(
            "bin_a",
            r#"
[package]
name = "bin_a"
version = "0.1.0"
edition = "2018"

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
edition = "2018"

[dependencies]
    "#,
        )
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), Some("bin_a".to_string())).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false)
        .unwrap();

    // Assert
    // root manifest + bin_a + lib_a (dependency)
    assert_eq!(3, skeleton.manifests.len());
    cook_directory
        .child("bin_a")
        .child("src")
        .child("main.rs")
        .assert("fn main() {}");
    cook_directory
        .child("lib_a")
        .child("src")
        .child("lib.rs")
        .assert("");
}
