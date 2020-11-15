use assert_fs::prelude::{FileTouch, FileWriteStr, PathChild, PathCreateDir};
use assert_fs::TempDir;
use chef::Skeleton;
use std::env;

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
    recipe_directory.child("src").create_dir_all().unwrap();
    recipe_directory
        .child("src")
        .child("main.rs")
        .touch()
        .unwrap();

    // Act
    let skeleton = Skeleton::derive(recipe_directory.path()).unwrap();
    let cook_directory = TempDir::new().unwrap();
    env::set_current_dir(cook_directory.path()).unwrap();
    skeleton.build_minimum_project().unwrap();

    // Assert
    assert_eq!(1, skeleton.manifests.len());
    let manifest = skeleton.manifests[0].clone();
    assert_eq!("Cargo.toml", manifest.relative_path.to_str().unwrap());
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
    project_b.child("src").child("main.rs").touch().unwrap();

    // Act
    let skeleton = Skeleton::derive(recipe_directory.path()).unwrap();
    let cook_directory = TempDir::new().unwrap();
    env::set_current_dir(cook_directory.path()).unwrap();
    skeleton.build_minimum_project().unwrap();

    // Assert
    assert_eq!(3, skeleton.manifests.len());
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
    let skeleton = Skeleton::derive(recipe_directory.path()).unwrap();
    let cook_directory = TempDir::new().unwrap();
    env::set_current_dir(cook_directory.path()).unwrap();
    skeleton.build_minimum_project().unwrap();

    // Assert
    assert_eq!(1, skeleton.manifests.len());
    let manifest = skeleton.manifests[0].clone();
    assert_eq!("Cargo.toml", manifest.relative_path.to_str().unwrap());
    assert!(cook_directory
        .child("benches")
        .child("basics.rs")
        .path()
        .exists())
}
