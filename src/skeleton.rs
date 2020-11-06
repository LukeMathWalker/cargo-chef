use crate::OptimisationProfile;
use anyhow::Context;
use fs_err as fs;
use globwalk::{GlobWalkerBuilder, WalkError};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Skeleton {
    pub manifests: Vec<Manifest>,
    pub lock_file: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// Relative path with respect to the project root.
    pub relative_path: PathBuf,
    pub contents: String,
}

impl Skeleton {
    /// Find all Cargo.toml files in `base_path` by traversing sub-directories recursively.
    pub fn derive<P: AsRef<Path>>(base_path: P) -> Result<Self, anyhow::Error> {
        let walker = GlobWalkerBuilder::new(&base_path, "/**/Cargo.toml")
            .build()
            .context("Failed to scan the files in the current directory.")?;
        let mut manifests = vec![];
        for manifest in walker {
            match manifest {
                Ok(manifest) => {
                    let absolute_path = manifest.path().to_path_buf();
                    let contents = fs::read_to_string(&absolute_path)?;

                    let mut parsed = cargo_manifest::Manifest::from_str(&contents)?;
                    // Required to detect bin/libs when the related section is omitted from the manifest
                    parsed.complete_from_path(&absolute_path)?;

                    // Workaround :(
                    // As suggested in issue #142 on toml-rs github repository
                    // First convert the Config instance to a toml Value,
                    // then serialize it to toml
                    let intermediate = toml::Value::try_from(parsed)?;
                    // The serialised contents might be different from the original manifest!
                    let contents = toml::to_string(&intermediate)?;

                    let relative_path = pathdiff::diff_paths(&absolute_path, &base_path)
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "Failed to compute relative path of manifest {:?}",
                                &absolute_path
                            )
                        })?;
                    manifests.push(Manifest {
                        relative_path,
                        contents,
                    });
                }
                Err(e) => match handle_walk_error(e) {
                    ErrorStrategy::Ignore => {}
                    ErrorStrategy::Crash(e) => {
                        return Err(e.into());
                    }
                },
            }
        }

        let lock_file = match fs::read_to_string(base_path.as_ref().join("Cargo.lock")) {
            Ok(lock) => Some(lock),
            Err(e) => {
                if std::io::ErrorKind::NotFound != e.kind() {
                    return Err(anyhow::Error::from(e).context("Failed to read Cargo.lock file."));
                }
                None
            }
        };
        Ok(Skeleton {
            manifests,
            lock_file,
        })
    }

    /// Given the manifests in the current skeleton, create the minimum set of files required to
    /// have a valid Rust project (i.e. write all manifests to disk and create dummy `lib.rs`,
    /// `main.rs` and `build.rs` files where needed).
    ///
    /// This function should be called on an empty canvas - i.e. an empty directory apart from
    /// the recipe file used to restore the skeleton.
    pub fn build_minimum_project(&self) -> Result<(), anyhow::Error> {
        // Save lockfile to disk, if available
        if let Some(lock_file) = &self.lock_file {
            fs::write("Cargo.lock", lock_file.as_str())?;
        }

        // Save all manifests to disks
        for manifest in &self.manifests {
            // Persist manifest
            let parent_directory = if let Some(parent_directory) = manifest.relative_path.parent() {
                fs::create_dir_all(&parent_directory)?;
                parent_directory.to_path_buf()
            } else {
                PathBuf::new()
            };
            fs::write(&manifest.relative_path, &manifest.contents)?;
            let parsed_manifest =
                cargo_manifest::Manifest::from_slice(manifest.contents.as_bytes())?;

            // Create dummy entrypoint files for all binaries
            for bin in &parsed_manifest.bin.unwrap_or_default() {
                // Relative to the manifest path
                let binary_relative_path = bin.path.as_deref().unwrap_or("src/main.rs");
                let binary_path = parent_directory.join(binary_relative_path);
                if let Some(parent_directory) = binary_path.parent() {
                    fs::create_dir_all(parent_directory)?;
                }
                fs::write(binary_path, "fn main() {}")?;
            }

            // Create dummy entrypoint files for for all libraries
            for lib in &parsed_manifest.lib {
                // Relative to the manifest path
                let lib_relative_path = lib.path.as_deref().unwrap_or("src/lib.rs");
                let lib_path = parent_directory.join(lib_relative_path);
                if let Some(parent_directory) = lib_path.parent() {
                    fs::create_dir_all(parent_directory)?;
                }
                fs::write(lib_path, "")?;
            }

            // Create dummy build script file if specified
            if let Some(package) = parsed_manifest.package {
                if let Some(build) = package.build {
                    if let cargo_manifest::Value::String(build_raw_path) = build {
                        // Relative to the manifest path
                        let build_relative_path = PathBuf::from(build_raw_path);
                        let build_path = parent_directory.join(build_relative_path);
                        if let Some(parent_directory) = build_path.parent() {
                            fs::create_dir_all(parent_directory)?;
                        }
                        fs::write(build_path, "fn main() {}")?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Scan the target directory and remove all compilation artifacts for libraries from the current
    /// workspace.
    /// Given the usage of dummy `lib.rs` files, keeping them around leads to funny compilation
    /// errors if they are a dependency of another project within the workspace.
    pub fn remove_compiled_dummy_libraries<P: AsRef<Path>>(
        &self,
        base_path: P,
        profile: OptimisationProfile,
        target: Option<String>,
    ) -> Result<(), anyhow::Error> {
        let mut target_directory = base_path.as_ref().join("target");
        if let Some(target) = target {
            target_directory = target_directory.join(target.as_str())
        }
        let target_directory = match profile {
            OptimisationProfile::Release => target_directory.join("release"),
            OptimisationProfile::Debug => target_directory.join("debug"),
        };

        for manifest in &self.manifests {
            let parsed_manifest =
                cargo_manifest::Manifest::from_slice(manifest.contents.as_bytes())?;

            for lib in &parsed_manifest.lib {
                let library_name = lib
                    .name
                    .clone()
                    .unwrap_or_else(|| parsed_manifest.package.as_ref().unwrap().name.to_owned())
                    .replace("-", "_");
                let walker =
                    GlobWalkerBuilder::new(&target_directory, format!("/**/lib{}*", library_name))
                        .build()?;
                for file in walker {
                    let file = file?;
                    fs::remove_file(file.path())?;
                }
            }
        }

        Ok(())
    }
}

/// What should we should when we encounter an issue while walking the current directory?
///
/// If `ErrorStrategy::Ignore`, just skip the file/directory and keep going.
/// If `ErrorStrategy::Crash`, stop exploring and return an error to the caller.
enum ErrorStrategy {
    Ignore,
    Crash(WalkError),
}

/// Ignore directory/files for which we don't have enough permissions to perform our scan.
#[must_use]
fn handle_walk_error(e: WalkError) -> ErrorStrategy {
    if let Some(inner) = e.io_error() {
        if std::io::ErrorKind::PermissionDenied == inner.kind() {
            log::warn!("Missing permission to read entry: {}\nSkipping.", inner);
            return ErrorStrategy::Ignore;
        }
    }
    ErrorStrategy::Crash(e)
}
