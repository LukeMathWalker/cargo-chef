use super::*;

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
    let skeleton = Skeleton::derive(project.path(), None).unwrap();
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
    let skeleton = Skeleton::derive(project.path(), None).unwrap();
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
        .build_minimum_project(cook_directory.path(), false)
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
        .build_minimum_project(cook_directory.path(), false)
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
    let skeleton = Skeleton::derive(project.path(), None).unwrap();
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
pub fn lints_are_removed_from_all_manifests() {
    // Arrange - workspace with [workspace.lints] and member with [lints] workspace = true
    // See https://github.com/LukeMathWalker/cargo-chef/issues/343
    let project = CargoWorkspace::new()
        .manifest(
            ".",
            r#"
[workspace]
members = [
    "crate_a",
]

[workspace.lints.rust]
missing_docs = "deny"
"#,
        )
        .lib_package(
            "crate_a",
            r#"
[package]
name = "crate_a"
version = "0.1.0"
edition = "2021"

[lints]
workspace = true

[dependencies]
"#,
        )
        .build();

    // Act
    let skeleton = Skeleton::derive(project.path(), None).unwrap();

    // Assert - lints should be stripped from all manifests
    for manifest in &skeleton.manifests {
        let parsed: toml::Value = toml::from_str(&manifest.contents).unwrap();

        // No top-level [lints]
        assert!(
            parsed.get("lints").is_none(),
            "Expected [lints] to be removed from manifest at {:?}, but found: {:?}",
            manifest.relative_path,
            parsed.get("lints")
        );

        // No [workspace.lints]
        if let Some(workspace) = parsed.get("workspace") {
            assert!(
                workspace.get("lints").is_none(),
                "Expected [workspace.lints] to be removed from manifest at {:?}, but found: {:?}",
                manifest.relative_path,
                workspace.get("lints")
            );
        }
    }
}
