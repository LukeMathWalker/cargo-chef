use std::collections::HashMap;
use std::path::{Path, PathBuf};

use assert_fs::prelude::*;
use assert_fs::TempDir;
use chef::Skeleton;
use expect_test::{expect, Expect};
use predicates::prelude::*;

#[test]
pub fn no_workspace() {
    // Arrange
    let project = CargoWorkspace::new()
        .manifest(
            ".",
            r#"
[package]
name = "test-dummy"
version = "0.1.0"
edition = "2018"

[[bin]]
name = "test-dummy"
path = "src/main.rs"

[dependencies]
"#,
        )
        .touch("src/main.rs")
        .touch("Cargo.lock")
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false, &[])
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
    let skeleton = Skeleton::derive(project.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), true, &[])
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
    let project = CargoWorkspace::new()
        .manifest(
            ".",
            r#"
[workspace]
members = [
    "src/project_a",
    "src/project_b",
]        
"#,
        )
        .bin_package(
            "src/project_a",
            r#"
[package]
name = "project_a"
version = "0.1.0"
edition = "2018"

[[bin]]
name = "test-dummy"
path = "src/main.rs"

[dependencies]
uuid = { version = "=0.8.0", features = ["v4"] }        
"#,
        )
        .lib_package(
            "src/project_b",
            r#"
[package]
name = "project_b"
version = "0.1.0"
edition = "2018"

[lib]
crate-type = ["cdylib"]

[dependencies]
uuid = { version = "=0.8.0", features = ["v4"] }        
"#,
        )
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false, &[])
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
    let skeleton = Skeleton::derive(project.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), true, &[])
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
    let project = CargoWorkspace::new()
        .lib_package(
            ".",
            r#"
[package]
name = "test-dummy"
version = "0.1.0"
edition = "2018"

[[bench]]
name = "basics"
harness = false

[dependencies]
"#,
        )
        .touch("benches/basics.rs")
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false, &[])
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
    let project = CargoWorkspace::new()
        .lib_package(
            ".",
            r#"
[package]
name = "test-dummy"
version = "0.1.0"
edition = "2018"

[[test]]
name = "foo"
"#,
        )
        .touch("tests/foo.rs")
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false, &[])
        .unwrap();

    // Assert
    assert_eq!(1, skeleton.manifests.len());
    let manifest = &skeleton.manifests[0];
    assert_eq!(Path::new("Cargo.toml"), manifest.relative_path);
    cook_directory.child("tests").child("foo.rs").assert("");

    // Act (no_std)
    let skeleton = Skeleton::derive(project.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), true, &[])
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
pub fn tests_no_harness() {
    // Arrange
    let project = CargoWorkspace::new()
        .lib_package(
            ".",
            r#"
[package]
name = "test-dummy"
version = "0.1.0"
edition = "2018"

[[test]]
name = "foo"
harness = false
"#,
        )
        .touch("tests/foo.rs")
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false, &[])
        .unwrap();

    cook_directory
        .child("tests")
        .child("foo.rs")
        .assert("fn main() {}");
}

#[test]
pub fn examples() {
    // Arrange
    let project = CargoWorkspace::new()
        .lib_package(
            ".",
            r#"
[package]
name = "test-dummy"
version = "0.1.0"
edition = "2018"

[[example]]
name = "foo"
"#,
        )
        .touch("examples/foo.rs")
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false, &[])
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
    let skeleton = Skeleton::derive(project.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), true, &[])
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
    let project = CargoWorkspace::new()
        .manifest(
            ".",
            r#"
[package]
name = "test-dummy"
version = "0.1.0"
edition = "2018"
"#,
        )
        .touch_multiple(&[
            "src/bin/a.rs",
            "src/bin/b.rs",
            "src/bin/c.rs",
            "src/bin/d.rs",
            "src/bin/e.rs",
            "src/bin/f.rs",
        ])
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), None).unwrap();

    // What we're testing is that auto-directories come back in the same order.
    // Since it's possible that the directories just happen to come back in the
    // same order randomly, we'll run this a few times to increase the
    // likelihood of triggering the problem if it exists.
    for _ in 0..5 {
        let skeleton2 = Skeleton::derive(project.path(), None).unwrap();
        assert_eq!(
            skeleton, skeleton2,
            "Skeletons of equal directories are not equal. Check [[bin]] ordering in manifest?"
        );
    }
}

#[test]
pub fn config_toml() {
    // Arrange
    let project = CargoWorkspace::new()
        .bin_package(
            ".",
            r#"
[package]
name = "test-dummy"
version = "0.1.0"
edition = "2018"

[dependencies]
"#,
        )
        .touch(".cargo/config.toml")
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false, &[])
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
    let project = CargoWorkspace::new()
        .bin_package(
            ".",
            r#"
[package]
name = "test-dummy"
version = "1.2.3"
edition = "2018"

[dependencies]
"#,
        )
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false, &[])
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
    let project = CargoWorkspace::new()
        .bin_package(
            ".",
            r#"
[package]
name = "test-dummy"
version = "1.2.3"
edition = "2018"

[dependencies]        
"#,
        )
        .file(
            "Cargo.lock",
            r#"
# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "test-dummy"
version = "1.2.3"
    "#,
        )
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false, &[])
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
    // project-a is named with a dash to test that such unnormalized name can be handled.
    let project = CargoWorkspace::new()
        .manifest(
            ".",
            r#"
[workspace]
members = [
    "src/project-a",
    "src/project_b",
]
"#,
        )
        .bin_package(
            "src/project-a",
            r#"
[package]
name = "project-a"
version = "1.2.3"
edition = "2018"

[[bin]]
name = "test-dummy"
path = "src/main.rs"

[dependencies]
either = { version = "=1.8.1" }        
"#,
        )
        .lib_package(
            "src/project_b",
            r#"
[package]
name = "project_b"
version = "4.5.6"
edition = "2018"

[lib]
crate-type = ["cdylib"]

[dependencies]
either = { version = "=1.8.1" }
project-a = { version = "1.2.3", path = "../project-a" }   
"#,
        )
        .file(
            "Cargo.lock",
            r#"
# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "either"
version = "1.8.1"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "7fcaabb2fef8c910e7f4c7ce9f67a1283a1715879a7c230ca9d6d1ae31f16d91"

[[package]]
name = "project-a"
version = "1.2.3"
dependencies = [
 "either",
]

[[package]]
name = "project_b"
version = "4.5.6"
dependencies = [
 "either",
 "project_a",
]
"#,
        )
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false, &[])
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
name = "either"
version = "1.8.1"
"#
    ));

    let first = skeleton.manifests[0].clone();
    check(
        &first.contents,
        expect_test::expect![[r#"
        [workspace]
        members = ["src/project-a", "src/project_b"]
    "#]],
    );
    let second = skeleton.manifests[1].clone();
    check(
        &second.contents,
        expect_test::expect![[r#"
            [[bin]]
            path = "src/main.rs"
            name = "test-dummy"
            plugin = false
            proc-macro = false
            edition = "2018"
            required-features = []

            [package]
            name = "project-a"
            edition = "2018"
            version = "0.0.1"

            [dependencies.either]
            version = "=1.8.1"
        "#]],
    );
    let third = skeleton.manifests[2].clone();
    check(
        &third.contents,
        expect_test::expect![[r#"
            [package]
            name = "project_b"
            edition = "2018"
            version = "0.0.1"

            [dependencies.either]
            version = "=1.8.1"

            [dependencies.project-a]
            version = "0.0.1"
            path = "../project-a"

            [lib]
            path = "src/lib.rs"
            name = "project_b"
            plugin = false
            proc-macro = false
            edition = "2018"
            required-features = []
            crate-type = ["cdylib"]
        "#]],
    );
}

#[test]
pub fn transitive_workspace_dependency_not_masked() {
    // Arrange
    let project = CargoWorkspace::new()
        .manifest(
            ".",
            r#"
[workspace]
members = [
    "src/project_a",
    "src/project_b",
]
"#,
        )
        .bin_package(
            "src/project_a",
            r#"
[package]
name = "project_a"
version = "2.2.2"
edition = "2018"

[[bin]]
name = "test-dummy"
path = "src/main.rs"

[dependencies]
either = { version = "=1.8.1" }
"#,
        )
        .lib_package(
            "src/project_b",
            r#"
[package]
name = "project_b"
version = "5.5.5"
edition = "2018"

[lib]
crate-type = ["cdylib"]

[dependencies]
"#,
        )
        .file(
            "Cargo.lock",
            r#"
# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "either"
version = "1.8.1"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "7fcaabb2fef8c910e7f4c7ce9f67a1283a1715879a7c230ca9d6d1ae31f16d91"
dependencies = [
 "project_b 5.5.5 (registry+https://github.com/rust-lang/crates.io-index)",
]

[[package]]
name = "project_a"
version = "2.2.2"
dependencies = [
 "either",
]

[[package]]
name = "project_b"
version = "5.5.5"

[[package]]
name = "project_b"
version = "5.5.5"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "7fcaabb2fef8c910e7f4c7ce9f67a1283a1715879a7c230ca9d6d1ae31f16d91"
"#,
        )
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false, &[])
        .unwrap();

    // Assert
    let lock_file = skeleton.lock_file.expect("there should be a lock_file");
    assert!(!lock_file.contains(
        r#"
[[package]]
name = "project_a"
version = "2.2.2"
"#
    ));
    assert!(lock_file.contains(
        r#"
[[package]]
name = "project_a"
version = "0.0.1"
"#
    ));
    assert!(lock_file.contains(
        r#"
[[package]]
name = "project_b"
version = "5.5.5"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "7fcaabb2fef8c910e7f4c7ce9f67a1283a1715879a7c230ca9d6d1ae31f16d91"
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
name = "either"
version = "1.8.1"
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
            [[bin]]
            path = "src/main.rs"
            name = "test-dummy"
            plugin = false
            proc-macro = false
            edition = "2018"
            required-features = []

            [package]
            name = "project_a"
            edition = "2018"
            version = "0.0.1"

            [dependencies.either]
            version = "=1.8.1"
        "#]],
    );
    let third = skeleton.manifests[2].clone();
    check(
        &third.contents,
        expect_test::expect![[r#"
            [package]
            name = "project_b"
            edition = "2018"
            version = "0.0.1"

            [dependencies]

            [lib]
            path = "src/lib.rs"
            name = "project_b"
            plugin = false
            proc-macro = false
            edition = "2018"
            required-features = []
            crate-type = ["cdylib"]
        "#]],
    );
}

#[test]
pub fn non_local_dependency_not_masked() {
    // Arrange
    let project = CargoWorkspace::new()
        .manifest(
            ".",
            r#"
[workspace]
members = [
    "binary",
    "without",
]
"#,
        )
        .bin_package(
            "binary",
            r#"
[package]
name = "binary"
version = "2.2.2"
edition = "2021"

[dependencies]
without = "=0.1.0"
without-local = { package = "without", path = "../without", version = "=0.1.0" }

"#,
        )
        .lib_package(
            "without",
            r#"
[package]
name = "without"
version = "0.1.0"
edition = "2021"

[dependencies]

"#,
        )
        .file(
            "Cargo.lock",
            r#"
# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "binary"
version = "2.2.2"
dependencies = [
 "without 0.1.0",
 "without 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)",
]

[[package]]
name = "without"
version = "0.1.0"

[[package]]
name = "without"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "3df10e9ed85b51fa3434bc5676eaa90479ce14ac3e101c8ce07e1bb5ef0b7255"

"#,
        )
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), None).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false, &[])
        .unwrap();

    // Assert
    let lock_file = skeleton.lock_file.expect("there should be a lock_file");
    println!("{lock_file}");
    assert!(lock_file.contains(
        r#"
[[package]]
name = "binary"
version = "0.0.1"
dependencies = ["without 0.1.0", "without 0.1.0 (registry+https://github.com/rust-lang/crates.io-index)"]

"#
    ));
    assert!(lock_file.contains(
        r#"
[[package]]
name = "without"
version = "0.0.1"
"#
    ));
    assert!(lock_file.contains(
        r#"
[[package]]
name = "without"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "3df10e9ed85b51fa3434bc5676eaa90479ce14ac3e101c8ce07e1bb5ef0b7255"
"#
    ));

    let first = skeleton.manifests[0].clone();
    check(
        &first.contents,
        expect_test::expect![[r#"
        [workspace]
        members = ["binary", "without"]
    "#]],
    );
    let second = skeleton.manifests[1].clone();
    check(
        &second.contents,
        expect_test::expect![[r#"
            [[bin]]
            path = "src/main.rs"
            name = "binary"
            plugin = false
            proc-macro = false
            edition = "2021"
            required-features = []

            [package]
            name = "binary"
            edition = "2021"
            version = "0.0.1"

            [dependencies]
            without = "=0.1.0"

            [dependencies.without-local]
            version = "0.0.1"
            path = "../without"
            package = "without"
        "#]],
    );
    let third = skeleton.manifests[2].clone();
    check(
        &third.contents,
        expect_test::expect![[r#"
            [package]
            name = "without"
            edition = "2021"
            version = "0.0.1"

            [dependencies]

            [lib]
            path = "src/lib.rs"
            name = "without"
            plugin = false
            proc-macro = false
            edition = "2021"
            required-features = []
            crate-type = ["lib"]
        "#]],
    );
}

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
            .find(|manifest| manifest.relative_path == std::path::PathBuf::from("Cargo.toml"))
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
        .build_minimum_project(cook_directory.path(), false, &[])
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
        .build_minimum_project(cook_directory.path(), false, &[])
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
        .build_minimum_project(cook_directory.path(), false, &[])
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
fn check(actual: &str, expect: Expect) {
    let actual = actual.to_string();
    expect.assert_eq(&actual);
}

#[derive(Default)]
struct CargoWorkspace {
    files: HashMap<PathBuf, String>,
}
impl CargoWorkspace {
    fn new() -> Self {
        Self::default()
    }

    fn manifest<P: AsRef<Path>>(&mut self, directory: P, content: &str) -> &mut Self {
        self.file(directory.as_ref().join("Cargo.toml"), content)
    }

    fn lib_package<P: AsRef<Path>>(&mut self, directory: P, content: &str) -> &mut Self {
        let directory = directory.as_ref();
        self.manifest(directory, content)
            .file(directory.join("src/lib.rs"), "")
    }

    fn bin_package<P: AsRef<Path>>(&mut self, directory: P, content: &str) -> &mut Self {
        let directory = directory.as_ref();
        self.manifest(directory, content)
            .file(directory.join("src/main.rs"), "")
    }

    fn file<P: AsRef<Path>>(&mut self, path: P, content: &str) -> &mut Self {
        let path = PathBuf::from(path.as_ref());

        assert!(self.files.insert(path, content.to_string()).is_none());
        self
    }

    fn touch<P: AsRef<Path>>(&mut self, path: P) -> &mut Self {
        self.file(path, "")
    }
    fn touch_multiple<P: AsRef<Path>>(&mut self, paths: &[P]) -> &mut Self {
        for path in paths {
            self.touch(path);
        }
        self
    }

    fn build(&mut self) -> BuiltWorkspace {
        let directory = TempDir::new().unwrap();
        for (file, content) in &self.files {
            let path = directory.join(file);
            let content = content.trim_start();
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        }
        BuiltWorkspace { directory }
    }
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

struct BuiltWorkspace {
    directory: TempDir,
}
impl BuiltWorkspace {
    fn path(&self) -> PathBuf {
        self.directory.canonicalize().unwrap()
    }
}
