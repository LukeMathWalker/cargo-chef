mod read;
mod target;
mod version_masking;

use crate::skeleton::target::{Target, TargetKind};
use crate::OptimisationProfile;
use anyhow::Context;
use cargo_manifest::Product;
use fs_err as fs;
use globwalk::GlobWalkerBuilder;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Skeleton {
    pub manifests: Vec<Manifest>,
    pub config_file: Option<String>,
    pub lock_file: Option<String>,
    pub rust_toolchain_file: Option<(RustToolchainFile, String)>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub enum RustToolchainFile {
    Bare,
    Toml,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// Relative path with respect to the project root.
    pub relative_path: PathBuf,
    pub contents: String,
    pub targets: Vec<Target>,
}

pub(in crate::skeleton) struct ParsedManifest {
    relative_path: PathBuf,
    contents: toml::Value,
    targets: Vec<Target>,
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
        let rust_toolchain_file = read::rust_toolchain(&base_path)?;

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
            rust_toolchain_file,
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

        // Save rust-toolchain or rust-toolchain.toml to disk, if available
        if let Some((file_kind, content)) = &self.rust_toolchain_file {
            let file_name = match file_kind {
                RustToolchainFile::Bare => "rust-toolchain",
                RustToolchainFile::Toml => "rust-toolchain.toml",
            };
            let path = base_path.join(file_name);
            fs::write(path, content.as_str())?;
        }

        // save config file to disk, if available
        if let Some(config_file) = &self.config_file {
            let parent_dir = base_path.join(".cargo");
            let config_file_path = parent_dir.join("config.toml");
            fs::create_dir_all(parent_dir)?;
            fs::write(config_file_path, config_file.as_str())?;
        }

        const NO_STD_ENTRYPOINT: &str = "#![no_std]
#![no_main]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
";
        const NO_STD_HARNESS_ENTRYPOINT: &str = r#"#![no_std]
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
"#;

        let get_test_like_entrypoint = |harness: bool| -> &str {
            match (no_std, harness) {
                (true, true) => NO_STD_HARNESS_ENTRYPOINT,
                (true, false) => NO_STD_ENTRYPOINT,
                (false, true) => "",
                (false, false) => "fn main() {}",
            }
        };

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

            let is_harness = |products: &Option<Vec<Product>>, name: &str| -> bool {
                products
                    .as_ref()
                    .and_then(|v| {
                        v.iter()
                            .find(|product| product.name.as_deref() == Some(name))
                            .map(|p| p.harness)
                    })
                    .unwrap_or(true)
            };

            // Create dummy entrypoints for all targets
            for target in &manifest.targets {
                let content = match target.kind {
                    TargetKind::BuildScript => "fn main() {}",
                    TargetKind::Bin | TargetKind::Example => {
                        if no_std {
                            NO_STD_ENTRYPOINT
                        } else {
                            "fn main() {}"
                        }
                    }
                    TargetKind::Lib { is_proc_macro } => {
                        if no_std && !is_proc_macro {
                            "#![no_std]"
                        } else {
                            ""
                        }
                    }
                    TargetKind::Bench => {
                        get_test_like_entrypoint(is_harness(&parsed_manifest.bench, &target.name))
                    }
                    TargetKind::Test => {
                        get_test_like_entrypoint(is_harness(&parsed_manifest.test, &target.name))
                    }
                };
                let path = parent_directory.join(&target.path);
                if let Some(dir) = path.parent() {
                    fs::create_dir_all(dir)?;
                }
                fs::write(&path, content)?;
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
            targets: manifest.targets,
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
///
/// Also deletes the `default-members` field because it does not play nicely
/// with a modified `members` field and has no effect on cooking the final receipe.
fn ignore_all_members_except(manifests: &mut [ParsedManifest], member: String) {
    let workspace_toml = manifests
        .iter_mut()
        .find(|manifest| manifest.relative_path == std::path::PathBuf::from("Cargo.toml"));

    if let Some(workspace) = workspace_toml.and_then(|toml| toml.contents.get_mut("workspace")) {
        if let Some(members) = workspace.get_mut("members") {
            *members = toml::Value::Array(vec![toml::Value::String(member)]);
        }
        if let Some(workspace) = workspace.as_table_mut() {
            workspace.remove("default-members");
        }
    }
}
