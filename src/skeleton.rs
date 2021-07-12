use crate::OptimisationProfile;
use anyhow::Context;
use fs_err as fs;
use globwalk::{GlobWalkerBuilder, WalkError};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::{
    borrow::BorrowMut,
    path::{Path, PathBuf},
};

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Skeleton {
    pub manifests: Vec<Manifest>,
    pub config_file: Option<String>,
    pub lock_file: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// Relative path with respect to the project root.
    pub relative_path: PathBuf,
    pub contents: String,
}

const CONST_VERSION: &str = "0.0.1";

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

                    let mut intermediate = toml::Value::try_from(parsed)?;

                    // ignore package.version for recipe
                    *intermediate
                        .get_mut("package")
                        .and_then(|v| v.get_mut("version"))
                        .borrow_mut() = Some(&mut toml::Value::String(CONST_VERSION.to_string()));

                    // Specifically, toml gives no guarantees to the ordering of the auto binaries
                    // in its results. We will manually sort these to ensure that the output
                    // manifest will match.
                    let bins = intermediate
                        .get_mut("bin")
                        .and_then(|bins| bins.as_array_mut());
                    if let Some(bins) = bins {
                        bins.sort_by(|bin_a, bin_b| {
                            let bin_a_path = bin_a
                                .as_table()
                                .and_then(|table| table.get("path"))
                                .and_then(|path| path.as_str())
                                .unwrap();
                            let bin_b_path = bin_b
                                .as_table()
                                .and_then(|table| table.get("path"))
                                .and_then(|path| path.as_str())
                                .unwrap();
                            bin_a_path.cmp(bin_b_path)
                        });
                    }

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

        // As we run primarly in Docker, assume to find config.toml at root level.
        let config_file = match fs::read_to_string(base_path.as_ref().join(".cargo/config.toml")) {
            Ok(config) => Some(config),
            Err(e) => {
                if std::io::ErrorKind::NotFound != e.kind() {
                    return Err(
                        anyhow::Error::from(e).context("Failed to read .cargo/config.toml file.")
                    );
                }
                None
            }
        };

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
            config_file,
            lock_file,
        })
    }

    /// Given the manifests in the current skeleton, create the minimum set of files required to
    /// have a valid Rust project (i.e. write all manifests to disk and create dummy `lib.rs`,
    /// `main.rs` and `build.rs` files where needed).
    ///
    /// This function should be called on an empty canvas - i.e. an empty directory apart from
    /// the recipe file used to restore the skeleton.
    pub fn build_minimum_project(
        &self,
        base_path: &Path,
        no_std: bool,
    ) -> Result<(), anyhow::Error> {
        // Save lockfile to disk, if available
        if let Some(lock_file) = &self.lock_file {
            let lock_file_path = base_path.join("Cargo.lock");
            fs::write(lock_file_path, lock_file.as_str())?;
        }

        // save config file to disk, if available
        if let Some(config_file) = &self.config_file {
            let parent_dir = base_path.join(".cargo");
            let config_file_path = parent_dir.join("config.toml");
            fs::create_dir_all(parent_dir)?;
            fs::write(config_file_path, config_file.as_str())?;
        }

        let no_std_entrypoint = "#![no_std]
#![no_main]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
";

        // Save all manifests to disks
        for manifest in &self.manifests {
            // Persist manifest
            let manifest_path = base_path.join(&manifest.relative_path);
            let parent_directory = if let Some(parent_directory) = manifest_path.parent() {
                fs::create_dir_all(&parent_directory)?;
                parent_directory.to_path_buf()
            } else {
                base_path.to_path_buf()
            };
            fs::write(&manifest_path, &manifest.contents)?;
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
                if no_std {
                    fs::write(binary_path, no_std_entrypoint)?;
                } else {
                    fs::write(binary_path, "fn main() {}")?;
                }
            }

            // Create dummy entrypoint files for for all libraries
            for lib in &parsed_manifest.lib {
                // Relative to the manifest path
                let lib_relative_path = lib.path.as_deref().unwrap_or("src/lib.rs");
                let lib_path = parent_directory.join(lib_relative_path);
                if let Some(parent_directory) = lib_path.parent() {
                    fs::create_dir_all(parent_directory)?;
                }
                if no_std && !lib.proc_macro {
                    fs::write(lib_path, "#![no_std]")?;
                } else {
                    fs::write(lib_path, "")?;
                }
            }

            // Create dummy entrypoint files for for all benchmarks
            for bench in &parsed_manifest.bench.unwrap_or_default() {
                // Relative to the manifest path
                let bench_name = bench.name.as_ref().context("Missing benchmark name.")?;
                let bench_relative_path = bench
                    .path
                    .clone()
                    .unwrap_or_else(|| format!("benches/{}.rs", bench_name));
                let bench_path = parent_directory.join(bench_relative_path);
                if let Some(parent_directory) = bench_path.parent() {
                    fs::create_dir_all(parent_directory)?;
                }
                fs::write(bench_path, "fn main() {}")?;
            }

            // Create dummy entrypoint files for for all tests
            for test in &parsed_manifest.test.unwrap_or_default() {
                // Relative to the manifest path
                let test_name = test.name.as_ref().context("Missing test name.")?;
                let test_relative_path = test
                    .path
                    .clone()
                    .unwrap_or_else(|| format!("tests/{}.rs", test_name));
                let test_path = parent_directory.join(test_relative_path);
                if let Some(parent_directory) = test_path.parent() {
                    fs::create_dir_all(parent_directory)?;
                }
                if no_std {
                    if test.harness {
                        fs::write(
                            test_path,
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
                        )?;
                    } else {
                        fs::write(test_path, no_std_entrypoint)?;
                    }
                } else if test.harness {
                    fs::write(test_path, "")?;
                } else {
                    fs::write(test_path, "fn main() {}")?;
                }
            }

            // Create dummy entrypoint files for for all examples
            for example in &parsed_manifest.example.unwrap_or_default() {
                // Relative to the manifest path
                let example_name = example.name.as_ref().context("Missing example name.")?;
                let example_relative_path = example
                    .path
                    .clone()
                    .unwrap_or_else(|| format!("examples/{}.rs", example_name));
                let example_path = parent_directory.join(example_relative_path);
                if let Some(parent_directory) = example_path.parent() {
                    fs::create_dir_all(parent_directory)?;
                }
                if no_std {
                    fs::write(example_path, no_std_entrypoint)?;
                } else {
                    fs::write(example_path, "fn main() {}")?;
                }
            }

            // Create dummy build script file if specified
            if let Some(package) = parsed_manifest.package {
                if let Some(cargo_manifest::Value::String(build_raw_path)) = package.build {
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
        Ok(())
    }

    /// Scan the target directory and remove all compilation artifacts for libraries and build
    /// scripts from the current workspace.
    /// Given the usage of dummy `lib.rs` and `build.rs` files, keeping them around leads to funny
    /// compilation errors.
    pub fn remove_compiled_dummies<P: AsRef<Path>>(
        &self,
        base_path: P,
        profile: OptimisationProfile,
        target: Option<String>,
        target_dir: Option<PathBuf>,
    ) -> Result<(), anyhow::Error> {
        let mut target_dir = match target_dir {
            None => base_path.as_ref().join("target"),
            Some(target_dir) => target_dir,
        };
        if let Some(target) = target {
            target_dir = target_dir.join(target.as_str())
        }
        let target_directory = match profile {
            OptimisationProfile::Release => target_dir.join("release"),
            OptimisationProfile::Debug => target_dir.join("debug"),
        };

        for manifest in &self.manifests {
            let parsed_manifest =
                cargo_manifest::Manifest::from_slice(manifest.contents.as_bytes())?;
            if let Some(package) = parsed_manifest.package.as_ref() {
                // Remove dummy libraries.
                for lib in &parsed_manifest.lib {
                    let library_name = lib.name.as_ref().unwrap_or(&package.name).replace("-", "_");
                    let walker = GlobWalkerBuilder::new(
                        &target_directory,
                        format!("/**/lib{}*", library_name),
                    )
                    .build()?;
                    for file in walker {
                        let file = file?;
                        if file.file_type().is_file() {
                            fs::remove_file(file.path())?;
                        } else if file.file_type().is_dir() {
                            fs::remove_dir_all(file.path())?;
                        }
                    }
                }

                // Remove dummy build.rs script artifacts.
                if package.build.is_some() {
                    let walker = GlobWalkerBuilder::new(
                        &target_directory,
                        format!("/build/{}-*/build[-_]script[-_]build*", package.name),
                    )
                    .build()?;
                    for file in walker {
                        let file = file?;
                        fs::remove_file(file.path())?;
                    }
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
