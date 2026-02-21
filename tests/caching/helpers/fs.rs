//! Filesystem helpers for scenario setup and copying.

use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn sync_project_to_cook_dir(project_root: &Path, cook_dir: &Path) {
    copy_dir_recursive(project_root, cook_dir, &["target", "recipe.json"]);
}

fn copy_dir_recursive(source: &Path, dest: &Path, ignore: &[&str]) {
    fs::create_dir_all(dest).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let file_type = entry.file_type().unwrap();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if ignore.contains(&name_str.as_ref()) {
            continue;
        }
        let from = entry.path();
        let to = dest.join(&name);
        if file_type.is_dir() {
            copy_dir_recursive(&from, &to, ignore);
        } else if file_type.is_file() {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::copy(&from, &to).unwrap();
        }
    }
}

pub(crate) fn write_file(path: PathBuf, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents.trim_start()).unwrap();
}
