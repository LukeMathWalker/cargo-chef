mod read;
mod version_masking;

use crate::OptimisationProfile;
use anyhow::Context;
use fs_err as fs;
use globwalk::GlobWalkerBuilder;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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

pub(in crate::skeleton) struct ParsedManifest {
    relative_path: PathBuf,
    contents: toml::Value,
}

impl Skeleton {
    /// Find all Cargo.toml files in `base_path` by traversing sub-directories recursively.
    pub fn derive<P: AsRef<Path>>(
        base_path: P,
        member: Option<String>,
    ) -> Result<Self, anyhow::Error> {
        let metadata = extract_cargo_metadata(base_path.as_ref())?;

        // Read relevant files from the filesystem
        let config_file = read::config(&base_path)?;
        let mut manifests = read::manifests(&base_path, metadata)?;
        if let Some(member) = member {
            ignore_all_members_except(&mut manifests, member);
        }

        let mut lock_file = read::lockfile(&base_path)?;

        version_masking::mask_local_crate_versions(&mut manifests, &mut lock_file);

        let lock_file = lock_file.map(|l| toml::to_string(&l)).transpose()?;

        let mut serialised_manifests = serialize_manifests(manifests)?;
        // We don't want an ordering issue (e.g. related to how files are read from the filesystem)
        // to make our skeleton generation logic non-reproducible - therefore we sort!
        serialised_manifests.sort_by_key(|m| m.relative_path.clone());

        Ok(Skeleton {
            manifests: serialised_manifests,
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
                fs::create_dir_all(parent_directory)?;
                parent_directory.to_path_buf()
            } else {
                base_path.to_path_buf()
            };
            fs::write(&manifest_path, &manifest.contents)?;
            let parsed_manifest =
                cargo_manifest::Manifest::from_slice(manifest.contents.as_bytes())?;

            let package_name = parsed_manifest.package.as_ref().map(|v| &v.name);
            // Create dummy entrypoint files for all binaries
            for bin in &parsed_manifest.bin.unwrap_or_default() {
                // Relative to the manifest path
                let binary_relative_path = bin.path.to_owned().unwrap_or_else(|| match &bin.name {
                    Some(name) if Some(name) != package_name => {
                        format!("src/bin/{}.rs", name)
                    }
                    _ => "src/main.rs".to_owned(),
                });
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
        target: Option<Vec<String>>,
        target_dir: Option<PathBuf>,
    ) -> Result<(), anyhow::Error> {
        let target_dir = match target_dir {
            None => base_path.as_ref().join("target"),
            Some(target_dir) => target_dir,
        };

        let profile = match profile {
            OptimisationProfile::Release => "release".to_string(),
            OptimisationProfile::Debug => "debug".to_string(),
            OptimisationProfile::Other(custom_profile) => custom_profile,
        };

        let target_directories: Vec<PathBuf> = target
            .map_or(vec![target_dir.clone()], |targets| {
                targets
                    .iter()
                    .map(|target| target_dir.join(target_str(target)))
                    .collect()
            })
            .iter()
            .map(|path| path.join(&profile))
            .collect();

        for manifest in &self.manifests {
            let parsed_manifest =
                cargo_manifest::Manifest::from_slice(manifest.contents.as_bytes())?;
            if let Some(package) = parsed_manifest.package.as_ref() {
                for target_directory in &target_directories {
                    // Remove dummy libraries.
                    for lib in &parsed_manifest.lib {
                        let library_name =
                            lib.name.as_ref().unwrap_or(&package.name).replace('-', "_");
                        let walker = GlobWalkerBuilder::from_patterns(
                            target_directory,
                            &[
                                format!("/**/lib{}.*", library_name),
                                format!("/**/lib{}-*", library_name),
                            ],
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
                            target_directory,
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
        }

        Ok(())
    }
}

/// If a custom target spec file is used,
/// (Part of the unstable cargo feature 'build-std'; c.f. https://doc.rust-lang.org/rustc/targets/custom.html )
/// the `--target` flag refers to a `.json` file in the current directory.
/// In this case, the actual name of the target is the value of `--target` without the `.json` suffix.
fn target_str(target: &str) -> &str {
    target.trim_end_matches(".json")
}

fn serialize_manifests(manifests: Vec<ParsedManifest>) -> Result<Vec<Manifest>, anyhow::Error> {
    let mut serialised_manifests = vec![];
    for manifest in manifests {
        // The serialised contents might be different from the original manifest!
        let contents = toml::to_string(&manifest.contents)?;
        serialised_manifests.push(Manifest {
            relative_path: manifest.relative_path,
            contents,
        });
    }
    Ok(serialised_manifests)
}

fn extract_cargo_metadata(path: &Path) -> Result<cargo_metadata::Metadata, anyhow::Error> {
    let mut cmd = cargo_metadata::MetadataCommand::new();
    cmd.current_dir(path);
    cmd.no_deps();

    cmd.exec().context("Cannot extract Cargo metadata")
}

/// If the top-level `Cargo.toml` has a `members` field, replace it with
/// a list consisting of just the specified member.
fn ignore_all_members_except(manifests: &mut [ParsedManifest], member: String) {
    let workspace_toml = manifests
        .iter_mut()
        .find(|manifest| manifest.relative_path == std::path::PathBuf::from("Cargo.toml"));

    if let Some(members) = workspace_toml
        .and_then(|toml| toml.contents.get_mut("workspace"))
        .and_then(|workspace| workspace.get_mut("members"))
    {
        *members = toml::Value::Array(vec![toml::Value::String(member)]);
    }
}
