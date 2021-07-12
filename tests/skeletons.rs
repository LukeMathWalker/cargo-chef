use assert_fs::prelude::{FileTouch, FileWriteStr, PathChild, PathCreateDir};
use assert_fs::TempDir;
use chef::Skeleton;

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
    let skeleton = Skeleton::derive(recipe_directory.path()).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false)
        .unwrap();

    // Assert
    assert_eq!(1, skeleton.manifests.len());
    let manifest = skeleton.manifests[0].clone();
    assert_eq!("Cargo.toml", manifest.relative_path.to_str().unwrap());
    assert!(cook_directory.child("src").child("main.rs").path().exists());
    assert!(cook_directory.child("Cargo.lock").path().exists());
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
    let skeleton = Skeleton::derive(recipe_directory.path()).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false)
        .unwrap();

    // Assert
    assert_eq!(3, skeleton.manifests.len());
    assert!(cook_directory
        .child("src")
        .child("project_a")
        .child("src")
        .child("main.rs")
        .path()
        .exists());
    assert!(cook_directory
        .child("src")
        .child("project_b")
        .child("src")
        .child("lib.rs")
        .path()
        .exists())
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
    skeleton
        .build_minimum_project(cook_directory.path(), false)
        .unwrap();

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
    let skeleton = Skeleton::derive(recipe_directory.path()).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false)
        .unwrap();

    // Assert
    assert_eq!(1, skeleton.manifests.len());
    let manifest = skeleton.manifests[0].clone();
    assert_eq!("Cargo.toml", manifest.relative_path.to_str().unwrap());
    assert!(cook_directory
        .child("tests")
        .child("foo.rs")
        .path()
        .exists())
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
    let skeleton = Skeleton::derive(recipe_directory.path()).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false)
        .unwrap();

    // Assert
    assert_eq!(1, skeleton.manifests.len());
    let manifest = skeleton.manifests[0].clone();
    assert_eq!("Cargo.toml", manifest.relative_path.to_str().unwrap());
    assert!(cook_directory
        .child("examples")
        .child("foo.rs")
        .path()
        .exists())
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
    let skeleton = Skeleton::derive(recipe_directory.path()).unwrap();

    // What we're testing is that auto-directories come back in the same order.
    // Since it's possible that the directories just happen to come back in the
    // same order randomly, we'll run this a few times to increase the
    // likelihood of triggering the problem if it exists.
    for _ in 0..5 {
        let skeleton2 = Skeleton::derive(recipe_directory.path()).unwrap();
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
    let skeleton = Skeleton::derive(recipe_directory.path()).unwrap();
    let cook_directory = TempDir::new().unwrap();
    skeleton
        .build_minimum_project(cook_directory.path(), false)
        .unwrap();

    // Assert
    assert_eq!(1, skeleton.manifests.len());
    let manifest = skeleton.manifests[0].clone();
    assert_eq!("Cargo.toml", manifest.relative_path.to_str().unwrap());
    assert!(cook_directory.child("src").child("main.rs").path().exists());
    assert!(cook_directory
        .child(".cargo")
        .child("config.toml")
        .path()
        .exists());
}
