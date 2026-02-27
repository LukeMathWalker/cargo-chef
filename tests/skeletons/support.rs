use std::collections::HashMap;
use std::path::{Path, PathBuf};

use assert_fs::TempDir;
use expect_test::Expect;

pub(crate) fn check(actual: &str, expect: Expect) {
    let actual = actual.to_string();
    expect.assert_eq(&actual);
}

#[derive(Default)]
pub(crate) struct CargoWorkspace {
    files: HashMap<PathBuf, String>,
}

impl CargoWorkspace {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn manifest<P: AsRef<Path>>(&mut self, directory: P, content: &str) -> &mut Self {
        self.file(directory.as_ref().join("Cargo.toml"), content)
    }

    pub(crate) fn lib_package<P: AsRef<Path>>(&mut self, directory: P, content: &str) -> &mut Self {
        let directory = directory.as_ref();
        self.manifest(directory, content)
            .file(directory.join("src/lib.rs"), "")
    }

    pub(crate) fn bin_package<P: AsRef<Path>>(&mut self, directory: P, content: &str) -> &mut Self {
        let directory = directory.as_ref();
        self.manifest(directory, content)
            .file(directory.join("src/main.rs"), "")
    }

    pub(crate) fn file<P: AsRef<Path>>(&mut self, path: P, content: &str) -> &mut Self {
        let path = PathBuf::from(path.as_ref());
        assert!(self.files.insert(path, content.to_string()).is_none());
        self
    }

    pub(crate) fn touch<P: AsRef<Path>>(&mut self, path: P) -> &mut Self {
        self.file(path, "")
    }

    pub(crate) fn touch_multiple<P: AsRef<Path>>(&mut self, paths: &[P]) -> &mut Self {
        for path in paths {
            self.touch(path);
        }
        self
    }

    pub(crate) fn build(&mut self) -> BuiltWorkspace {
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

pub(crate) struct BuiltWorkspace {
    directory: TempDir,
}

impl BuiltWorkspace {
    pub(crate) fn path(&self) -> PathBuf {
        self.directory.canonicalize().unwrap()
    }
}
