use std::path::Path;

use assert_fs::prelude::*;
use assert_fs::TempDir;
use chef::Skeleton;
use expect_test::Expect;
use predicates::prelude::*;

#[test]
pub fn no_workspace() {
    // Arrange
    let content = r#"
[package]
name = "test-dummy"
version = "0.1.0"
edition = "2018"

[[bin]]
name = "test-dummy"
path = "src/main.rs"

[dependencies]
    "#;

    let recipe_directory = TempDir::new().unwrap();
    let manifest = recipe_directory.child("Cargo.toml");
    manifest.write_str(content).unwrap();
    recipe_directory.child("Cargo.lock").touch().unwrap();
    recipe_directory.child("src").create_dir_all().unwrap();
    recipe_directory
        .child("src")
        .child("main.rs")
        .touch()
        .unwrap();

    // Act
    let skeleton = Skeleton::derive(recipe_directory.path(), None).unwrap();
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

    // Act (no_std)
    let skeleton = Skeleton::derive(recipe_directory.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), true)
        .unwrap();

    // Assert (no_std)
    assert_eq!(1, skeleton.manifests.len());
    let manifest = &skeleton.manifests[0];
    assert_eq!(Path::new("Cargo.toml"), manifest.relative_path);
    cook_directory.child("src").child("main.rs").assert(
        r#"#![no_std]
#![no_main]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
"#,
    );
    cook_directory
        .child("Cargo.lock")
        .assert(predicate::path::exists());
}

#[test]
pub fn workspace() {
    // Arrange
    let workspace_content = r#"
[workspace]

members = [
    "src/project_a",
    "src/project_b",
]
    "#;

    let first_content = r#"
[package]
name = "project_a"
version = "0.1.0"
edition = "2018"

[[bin]]
name = "test-dummy"
path = "src/main.rs"

[dependencies]
uuid = { version = "=0.8.0", features = ["v4"] }
    "#;

    let second_content = r#"
[package]
name = "project_b"
version = "0.1.0"
edition = "2018"

[lib]
crate-type = ["cdylib"]

[dependencies]
uuid = { version = "=0.8.0", features = ["v4"] }
    "#;

    let recipe_directory = TempDir::new().unwrap();
    let manifest = recipe_directory.child("Cargo.toml");
    manifest.write_str(workspace_content).unwrap();
    let src = recipe_directory.child("src");
    src.create_dir_all().unwrap();

    let project_a = src.child("project_a");
    project_a
        .child("Cargo.toml")
        .write_str(first_content)
        .unwrap();
    project_a.child("src").create_dir_all().unwrap();
    project_a.child("src").child("main.rs").touch().unwrap();

    let project_b = src.child("project_b");
    project_b
        .child("Cargo.toml")
        .write_str(second_content)
        .unwrap();
    project_b.child("src").create_dir_all().unwrap();
    project_b.child("src").child("lib.rs").touch().unwrap();

    // Act
    let skeleton = Skeleton::derive(recipe_directory.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false)
        .unwrap();

    // Assert
    assert_eq!(3, skeleton.manifests.len());
    cook_directory
        .child("src")
        .child("project_a")
        .child("src")
        .child("main.rs")
        .assert("fn main() {}");
    cook_directory
        .child("src")
        .child("project_b")
        .child("src")
        .child("lib.rs")
        .assert("");

    // Act (no_std)
    let skeleton = Skeleton::derive(recipe_directory.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), true)
        .unwrap();

    // Assert (no_std)
    assert_eq!(3, skeleton.manifests.len());
    cook_directory
        .child("src")
        .child("project_a")
        .child("src")
        .child("main.rs")
        .assert(
            r#"#![no_std]
#![no_main]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
"#,
        );
    cook_directory
        .child("src")
        .child("project_b")
        .child("src")
        .child("lib.rs")
        .assert("#![no_std]");
}

#[test]
pub fn benches() {
    // Arrange
    let content = r#"
[package]
name = "test-dummy"
version = "0.1.0"
edition = "2018"

[lib]
name = "test-dummy"

[[bench]]
name = "basics"
harness = false

[dependencies]
    "#;

    let recipe_directory = TempDir::new().unwrap();
    let manifest = recipe_directory.child("Cargo.toml");
    manifest.write_str(content).unwrap();
    recipe_directory.child("src").create_dir_all().unwrap();
    recipe_directory
        .child("src")
        .child("lib.rs")
        .touch()
        .unwrap();
    recipe_directory.child("benches").create_dir_all().unwrap();
    recipe_directory
        .child("benches")
        .child("basics.rs")
        .touch()
        .unwrap();

    // Act
    let skeleton = Skeleton::derive(recipe_directory.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false)
        .unwrap();

    // Assert
    assert_eq!(1, skeleton.manifests.len());
    let manifest = &skeleton.manifests[0];
    assert_eq!(Path::new("Cargo.toml"), manifest.relative_path);
    cook_directory
        .child("benches")
        .child("basics.rs")
        .assert("fn main() {}");

    // no_std benches are not a thing yet
}

#[test]
pub fn tests() {
    // Arrange
    let content = r#"
[package]
name = "test-dummy"
version = "0.1.0"
edition = "2018"

[[test]]
name = "foo"
    "#;

    let recipe_directory = TempDir::new().unwrap();
    let manifest = recipe_directory.child("Cargo.toml");
    manifest.write_str(content).unwrap();
    recipe_directory.child("src").create_dir_all().unwrap();
    recipe_directory
        .child("src")
        .child("lib.rs")
        .touch()
        .unwrap();
    recipe_directory.child("tests").create_dir_all().unwrap();
    recipe_directory
        .child("tests")
        .child("foo.rs")
        .touch()
        .unwrap();

    // Act
    let skeleton = Skeleton::derive(recipe_directory.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false)
        .unwrap();

    // Assert
    assert_eq!(1, skeleton.manifests.len());
    let manifest = &skeleton.manifests[0];
    assert_eq!(Path::new("Cargo.toml"), manifest.relative_path);
    cook_directory.child("tests").child("foo.rs").assert("");

    // Act (no_std)
    let skeleton = Skeleton::derive(recipe_directory.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), true)
        .unwrap();

    // Assert (no_std)
    assert_eq!(1, skeleton.manifests.len());
    let manifest = &skeleton.manifests[0];
    assert_eq!(Path::new("Cargo.toml"), manifest.relative_path);
    cook_directory.child("tests").child("foo.rs").assert(
        r#"#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_runner)]

#[no_mangle]
pub extern "C" fn _init() {}

fn test_runner(_: &[&dyn Fn()]) {}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
"#,
    );
}

#[test]
pub fn examples() {
    // Arrange
    let content = r#"
[package]
name = "test-dummy"
version = "0.1.0"
edition = "2018"

[[example]]
name = "foo"
    "#;

    let recipe_directory = TempDir::new().unwrap();
    let manifest = recipe_directory.child("Cargo.toml");
    manifest.write_str(content).unwrap();
    recipe_directory.child("src").create_dir_all().unwrap();
    recipe_directory
        .child("src")
        .child("lib.rs")
        .touch()
        .unwrap();
    recipe_directory.child("examples").create_dir_all().unwrap();
    recipe_directory
        .child("examples")
        .child("foo.rs")
        .touch()
        .unwrap();

    // Act
    let skeleton = Skeleton::derive(recipe_directory.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false)
        .unwrap();

    // Assert
    assert_eq!(1, skeleton.manifests.len());
    let manifest = &skeleton.manifests[0];
    assert_eq!(Path::new("Cargo.toml"), manifest.relative_path);
    cook_directory
        .child("examples")
        .child("foo.rs")
        .assert("fn main() {}");

    // Act (no_std)
    let skeleton = Skeleton::derive(recipe_directory.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), true)
        .unwrap();

    // Assert (no_std)
    assert_eq!(1, skeleton.manifests.len());
    let manifest = &skeleton.manifests[0];
    assert_eq!(Path::new("Cargo.toml"), manifest.relative_path);
    cook_directory.child("examples").child("foo.rs").assert(
        r#"#![no_std]
#![no_main]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
"#,
    );
}

#[test]
pub fn test_auto_bin_ordering() {
    // Arrange
    let content = r#"
[package]
name = "test-dummy"
version = "0.1.0"
edition = "2018"
"#;
    let recipe_directory = TempDir::new().unwrap();
    let manifest = recipe_directory.child("Cargo.toml");
    manifest.write_str(content).unwrap();
    let bin_dir = recipe_directory.child("src").child("bin");
    bin_dir.create_dir_all().unwrap();
    bin_dir.child("a.rs").touch().unwrap();
    bin_dir.child("b.rs").touch().unwrap();
    bin_dir.child("c.rs").touch().unwrap();
    bin_dir.child("d.rs").touch().unwrap();
    bin_dir.child("e.rs").touch().unwrap();
    bin_dir.child("f.rs").touch().unwrap();

    // Act
    let skeleton = Skeleton::derive(recipe_directory.path(), None).unwrap();

    // What we're testing is that auto-directories come back in the same order.
    // Since it's possible that the directories just happen to come back in the
    // same order randomly, we'll run this a few times to increase the
    // likelihood of triggering the problem if it exists.
    for _ in 0..5 {
        let skeleton2 = Skeleton::derive(recipe_directory.path(), None).unwrap();
        assert_eq!(
            skeleton, skeleton2,
            "Skeletons of equal directories are not equal. Check [[bin]] ordering in manifest?"
        );
    }
}

#[test]
pub fn config_toml() {
    // Arrange
    let content = r#"
        [package]
        name = "test-dummy"
        version = "0.1.0"
        edition = "2018"

        [dependencies]
            "#;

    let recipe_directory = TempDir::new().unwrap();
    let manifest = recipe_directory.child("Cargo.toml");
    manifest.write_str(content).unwrap();
    recipe_directory.child(".cargo").create_dir_all().unwrap();
    recipe_directory
        .child(".cargo")
        .child("config.toml")
        .touch()
        .unwrap();
    recipe_directory.child("src").create_dir_all().unwrap();
    recipe_directory
        .child("src")
        .child("main.rs")
        .touch()
        .unwrap();

    // Act
    let skeleton = Skeleton::derive(recipe_directory.path(), None).unwrap();
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
        .child(".cargo")
        .child("config.toml")
        .assert(predicate::path::exists());
}

#[test]
pub fn version() {
    // Arrange
    let content = r#"
        [package]
        name = "test-dummy"
        version = "1.2.3"
        edition = "2018"

        [dependencies]
            "#;

    let recipe_directory = TempDir::new().unwrap();
    let manifest = recipe_directory.child("Cargo.toml");
    manifest.write_str(content).unwrap();
    recipe_directory.child(".cargo").create_dir_all().unwrap();
    recipe_directory
        .child(".cargo")
        .child("config.toml")
        .touch()
        .unwrap();
    recipe_directory.child("src").create_dir_all().unwrap();
    recipe_directory
        .child("src")
        .child("main.rs")
        .touch()
        .unwrap();

    // Act
    let skeleton = Skeleton::derive(recipe_directory.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false)
        .unwrap();

    // Assert
    assert_eq!(1, skeleton.manifests.len());
    let manifest = skeleton.manifests[0].clone();
    assert!(manifest.contents.contains(r#"version = "0.0.1""#));
    assert!(!manifest.contents.contains(r#"version = "1.2.3""#));
}

#[test]
pub fn version_lock() {
    // Arrange
    let content = r#"
[package]
name = "test-dummy"
version = "1.2.3"
edition = "2018"

[dependencies]
    "#;
    let lockfile = r#"
# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "test-dummy"
version = "1.2.3"
    "#;

    let recipe_directory = TempDir::new().unwrap();
    let manifest = recipe_directory.child("Cargo.toml");
    manifest.write_str(content).unwrap();
    let lock_file = recipe_directory.child("Cargo.lock");
    lock_file.write_str(lockfile).unwrap();
    recipe_directory.child(".cargo").create_dir_all().unwrap();
    recipe_directory
        .child(".cargo")
        .child("config.toml")
        .touch()
        .unwrap();
    recipe_directory.child("src").create_dir_all().unwrap();
    recipe_directory
        .child("src")
        .child("main.rs")
        .touch()
        .unwrap();

    // Act
    let skeleton = Skeleton::derive(recipe_directory.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false)
        .unwrap();

    // Assert
    let lock_file = skeleton.lock_file.expect("there should be a lock_file");
    assert!(!lock_file.contains(
        r#"
[[package]]
name = "test-dummy"
version = "1.2.3"
"#
    ));
    assert!(lock_file.contains(
        r#"
[[package]]
name = "test-dummy"
version = "0.0.1"
"#
    ));
}

#[test]
pub fn workspace_version_lock() {
    // Arrange
    let workspace_content = r#"
[workspace]

members = [
    "src/project_a",
    "src/project_b",
]
    "#;

    let first_content = r#"
[package]
name = "project-a"
version = "1.2.3"
edition = "2018"

[[bin]]
name = "test-dummy"
path = "src/main.rs"

[dependencies]
uuid = { version = "=0.8.0", features = ["v4"] }
    "#;

    let second_content = r#"
[package]
name = "project_b"
version = "4.5.6"
edition = "2018"

[lib]
crate-type = ["cdylib"]

[dependencies]
uuid = { version = "=0.8.0", features = ["v4"] }
project_a = { version = "0.0.1", path = "../project_a" }
    "#;

    let lockfile = r#"
# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "project-a"
version = "1.2.3"
dependencies = [
    "uuid",
]

[[package]]
name = "project_b"
version = "4.5.6"
dependencies = [
    "uuid",
    "project-a"
]

[[package]]
name = "uuid"
version = "0.8.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "bc5cf98d8186244414c848017f0e2676b3fcb46807f6668a97dfe67359a3c4b7"
    "#;

    let recipe_directory = TempDir::new().unwrap();
    let manifest = recipe_directory.child("Cargo.toml");
    manifest.write_str(workspace_content).unwrap();
    let lock_file = recipe_directory.child("Cargo.lock");
    lock_file.write_str(lockfile).unwrap();
    let src = recipe_directory.child("src");
    src.create_dir_all().unwrap();

    let project_a = src.child("project_a");
    project_a
        .child("Cargo.toml")
        .write_str(first_content)
        .unwrap();
    project_a.child("src").create_dir_all().unwrap();
    project_a.child("src").child("main.rs").touch().unwrap();

    let project_b = src.child("project_b");
    project_b
        .child("Cargo.toml")
        .write_str(second_content)
        .unwrap();
    project_b.child("src").create_dir_all().unwrap();
    project_b.child("src").child("lib.rs").touch().unwrap();

    // Act
    let skeleton = Skeleton::derive(recipe_directory.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false)
        .unwrap();

    // Assert
    let lock_file = skeleton.lock_file.expect("there should be a lock_file");
    assert!(!lock_file.contains(
        r#"
[[package]]
name = "project-a"
version = "1.2.3"
"#
    ));
    assert!(lock_file.contains(
        r#"
[[package]]
name = "project-a"
version = "0.0.1"
"#
    ));
    assert!(!lock_file.contains(
        r#"
[[package]]
name = "project_b"
version = "4.5.6"
"#
    ));
    assert!(lock_file.contains(
        r#"
[[package]]
name = "project_b"
version = "0.0.1"
"#
    ));
    assert!(lock_file.contains(
        r#"
[[package]]
name = "uuid"
version = "0.8.0"
"#
    ));

    let first = skeleton.manifests[0].clone();
    check(
        &first.contents,
        expect_test::expect![[r#"
        [workspace]
        members = ["src/project_a", "src/project_b"]
    "#]],
    );
    let second = skeleton.manifests[1].clone();
    check(
        &second.contents,
        expect_test::expect![[r#"
            bench = []
            test = []
            example = []

            [[bin]]
            path = "src/main.rs"
            name = "test-dummy"
            test = true
            doctest = true
            bench = true
            doc = true
            plugin = false
            proc-macro = false
            harness = true
            required-features = []

            [package]
            name = "project-a"
            edition = "2018"
            version = "0.0.1"
            authors = []
            keywords = []
            categories = []
            autobins = true
            autoexamples = true
            autotests = true
            autobenches = true
            publish = true
            [dependencies.uuid]
            version = "=0.8.0"
            features = ["v4"]
            optional = false
        "#]],
    );
    let third = skeleton.manifests[2].clone();
    check(
        &third.contents,
        expect_test::expect![[r#"
            bin = []
            bench = []
            test = []
            example = []

            [package]
            name = "project_b"
            edition = "2018"
            version = "0.0.1"
            authors = []
            keywords = []
            categories = []
            autobins = true
            autoexamples = true
            autotests = true
            autobenches = true
            publish = true
            [dependencies.project_a]
            version = "0.0.1"
            path = "../project_a"
            features = []
            optional = false

            [dependencies.uuid]
            version = "=0.8.0"
            features = ["v4"]
            optional = false

            [lib]
            test = true
            doctest = true
            bench = true
            doc = true
            plugin = false
            proc-macro = false
            harness = true
            required-features = []
            crate-type = ["cdylib"]
        "#]],
    );
}

#[test]
pub fn ignore_vendored_directory() {
    // Arrange
    let content = r#"
[package]
name = "test-dummy"
version = "1.2.3"
edition = "2018"

[dependencies]
rocket = "0.5.0-rc.1"
    "#;
    let cargo_config = r#"
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
    "#;

    let rocket_cargo_toml = r#"[package]
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
version = "0.3.2""#;

    let rocket_dep_cargo_toml = r#"[package]
edition = "2018"
name = "rocket_dep"
version = "0.3.2"
authors = ["Test author"]
description = "sample package representing all of rocket's dependencies""#;

    let recipe_directory = TempDir::new().unwrap();
    let manifest = recipe_directory.child("Cargo.toml");
    manifest.write_str(content).unwrap();
    recipe_directory.child(".cargo").create_dir_all().unwrap();
    recipe_directory
        .child(".cargo")
        .child("config.toml")
        .write_str(cargo_config)
        .unwrap();
    recipe_directory.child("src").create_dir_all().unwrap();
    recipe_directory
        .child("src")
        .child("main.rs")
        .touch()
        .unwrap();

    let vendored = recipe_directory.child("vendor");
    let rocket = vendored.child("rocket");
    let sample_rocket_dep = vendored.child("rocket_dep");
    rocket.create_dir_all().unwrap();
    sample_rocket_dep.create_dir_all().unwrap();

    rocket
        .child("Cargo.toml")
        .write_str(rocket_cargo_toml)
        .unwrap();

    sample_rocket_dep
        .child("Cargo.toml")
        .write_str(rocket_dep_cargo_toml)
        .unwrap();

    // Act
    let skeleton = Skeleton::derive(recipe_directory.path(), None).unwrap();

    // Assert
    assert_eq!(1, skeleton.manifests.len());
}

#[test]
pub fn specify_member_in_workspace() {
    // Arrange
    let workspace_content = r#"
[workspace]

members = [
    "backend",
    "ci",
]
    "#;

    let first_content = r#"
[package]
name = "backend"
version = "0.1.0"
edition = "2018"
    "#;

    let recipe_directory = TempDir::new().unwrap();
    let manifest = recipe_directory.child("Cargo.toml");
    manifest.write_str(workspace_content).unwrap();
    let backend = recipe_directory.child("backend");
    backend.create_dir_all().unwrap();

    backend
        .child("Cargo.toml")
        .write_str(first_content)
        .unwrap();
    backend.child("src").create_dir_all().unwrap();
    backend.child("src").child("main.rs").touch().unwrap();

    // Act
    let skeleton = Skeleton::derive(recipe_directory.path(), "backend".to_string().into()).unwrap();

    // Assert
    recipe_directory.child("Cargo.toml").assert(
        r#"[workspace]
members = ["backend"]
"#,
    );
}

fn check(actual: &str, expect: Expect) {
    let actual = actual.to_string();
    expect.assert_eq(&actual);
}
