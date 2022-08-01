use assert_fs::prelude::{FileTouch, FileWriteStr, PathChild, PathCreateDir};
use assert_fs::TempDir;
use chef::Recipe;

fn quick_recipe(content: &str) -> Recipe {
    let recipe_directory = TempDir::new().unwrap();
    let manifest = recipe_directory.child("Cargo.toml");
    manifest.write_str(content).unwrap();
    let bin_dir = recipe_directory.child("src").child("bin");
    let test_dir = recipe_directory.child("tests");
    bin_dir.create_dir_all().unwrap();
    test_dir.create_dir_all().unwrap();
    for filename in &["f.rs", "e.rs", "d.rs", "c.rs", "b.rs", "a.rs"] {
        bin_dir.child(filename).touch().unwrap();
        test_dir.child(filename).touch().unwrap();
    }
    Recipe::prepare(recipe_directory.path().into(), None).unwrap()
}

#[test]
fn test_recipe_is_deterministic() {
    let content = r#"
[package]
name = "test-dummy"
version = "0.1.0"
edition = "2018"

[[bin]]
name = "bin2"
path = "some-path.rs"

[[bin]]
name = "bin1"
path = "some-other-path.rs"

[[test]]
name = "test2"
path = "some-other-path.rs"

[[test]]
name = "test1"
path = "some-path.rs"
"#;
    let recipe = quick_recipe(content);
    let recipe_json = serde_json::to_string(&recipe).unwrap();
    // construct a recipe a bunch more times and assert that each time
    // it is equal to the first (both the object and the json serialization)
    for _ in 0..5 {
        let recipe2 = quick_recipe(content);
        let recipe2_json = serde_json::to_string(&recipe).unwrap();
        assert_eq!(
            recipe, recipe2,
            "recipes of equal directories are not equal"
        );
        assert_eq!(
            recipe_json, recipe2_json,
            "recipe jsons of equal directories are not equal"
        );
    }
}
