//! Logic to read all the files required to build a caching layer for a project.
use super::ParsedManifest;
use crate::skeleton::target::{Target, TargetKind};
use crate::RustToolchainFile;
use cargo_metadata::{Metadata, Package};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub(super) fn config<P: AsRef<Path>>(base_path: &P) -> Result<Option<String>, anyhow::Error> {
    // Given that we run primarily in Docker, assume to find config or config.toml at root level.
    // We give priority to config over config.toml since this is cargo's default behavior.

    let file_contents = |file: &str| {
        fs::read_to_string(
            base_path
                .as_ref()
                .join(".cargo")
                .join(file)
                .into_os_string(),
        )
    };

    let config = file_contents("config").or_else(|_| file_contents("config.toml"));

    match config {
        Ok(config) => Ok(Some(config)),
        Err(e) => {
            if std::io::ErrorKind::NotFound != e.kind() {
                return Err(
                    anyhow::Error::from(e).context("Failed to read .cargo/config.toml file.")
                );
            }
            Ok(None)
        }
    }
}

pub(super) fn manifests<P: AsRef<Path>>(
    base_path: &P,
    metadata: &Metadata,
) -> Result<Vec<ParsedManifest>, anyhow::Error> {
    let mut packages: BTreeMap<PathBuf, BTreeSet<Target>> = metadata
        .workspace_packages()
        .iter()
        .copied()
        .chain(metadata.root_package())
        .map(|p| {
            (
                p.manifest_path.clone().into_std_path_buf(),
                gather_targets(p),
            )
        })
        .collect();

    if metadata.root_package().is_none() {
        // At the root, there might be a Cargo.toml manifest with a [workspace] section.
        // However, if this root manifest doesn't contain [package], it is not considered a package
        // by cargo metadata. Therefore, we have to add it manually.
        // Workspaces currently cannot be nested, so this should only happen at the root.
        packages.insert(base_path.as_ref().join("Cargo.toml"), Default::default());
    }

    let mut manifests = vec![];
    for (absolute_path, targets) in packages {
        let contents = fs::read_to_string(&absolute_path)?;

        let mut parsed = cargo_manifest::Manifest::from_str(&contents)?;
        // The completions are relevant for our analysis, but we shouldn't
        // include them in the final output.
        let before_completions = toml::Value::try_from(&parsed)?;

        // Required to detect bin/libs when the related section is omitted from the manifest
        parsed.complete_from_path(&absolute_path)?;

        let mut intermediate = toml::Value::try_from(parsed)?;

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
                    .and_then(|table| table.get("path").or_else(|| table.get("name")))
                    .and_then(|path| path.as_str())
                    .unwrap();
                let bin_b_path = bin_b
                    .as_table()
                    .and_then(|table| table.get("path").or_else(|| table.get("name")))
                    .and_then(|path| path.as_str())
                    .unwrap();
                bin_a_path.cmp(bin_b_path)
            });
        }

        let relative_path = pathdiff::diff_paths(&absolute_path, base_path).ok_or_else(|| {
            anyhow::anyhow!(
                "Failed to compute relative path of manifest {:?}",
                &absolute_path
            )
        })?;

        manifests.push(ParsedManifest {
            relative_path,
            contents: before_completions,
            targets: targets.into_iter().collect(),
        });
    }

    Ok(manifests)
}

fn gather_targets(package: &Package) -> BTreeSet<Target> {
    let manifest_path = package.manifest_path.clone().into_std_path_buf();
    let root_dir = manifest_path.parent().unwrap();
    package
        .targets
        .iter()
        .map(|target| {
            let relative_path = pathdiff::diff_paths(&target.src_path, root_dir).unwrap();
            let kind = if target.is_bench() {
                TargetKind::Bench
            } else if target.is_example() {
                TargetKind::Example
            } else if target.is_test() {
                TargetKind::Test
            } else if target.is_bin() {
                TargetKind::Bin
            } else if target.is_custom_build() {
                TargetKind::BuildScript
            } else {
                // If a library has custom crate type (e.g. "cdylib"), it's kind will be "cdylib"
                // instead of just "lib". Therefore, we assume that this target is a library.
                TargetKind::Lib {
                    is_proc_macro: target
                        .crate_types
                        .iter()
                        .any(|t| t.as_str() == "proc-macro"),
                }
            };

            Target {
                path: relative_path,
                kind,
                name: target.name.clone(),
            }
        })
        .collect()
}

pub(super) fn lockfile<P: AsRef<Path>>(
    base_path: &P,
) -> Result<Option<toml::Value>, anyhow::Error> {
    match fs::read_to_string(base_path.as_ref().join("Cargo.lock")) {
        Ok(lock) => {
            let lock: toml::Value = toml::from_str(&lock)?;
            Ok(Some(lock))
        }
        Err(e) => {
            if std::io::ErrorKind::NotFound != e.kind() {
                return Err(anyhow::Error::from(e).context("Failed to read Cargo.lock file."));
            }
            Ok(None)
        }
    }
}

pub(super) fn rust_toolchain<P: AsRef<Path>>(
    base_path: &P,
) -> Result<Option<(RustToolchainFile, String)>, anyhow::Error> {
    // `rust-toolchain` takes precedence over `rust-toolchain.toml`
    if let Some(file) = read_rust_toolchain(&base_path.as_ref().join("rust-toolchain"))? {
        return Ok(Some((RustToolchainFile::Bare, file)));
    }

    if let Some(file) = read_rust_toolchain(&base_path.as_ref().join("rust-toolchain.toml"))? {
        return Ok(Some((RustToolchainFile::Toml, file)));
    }

    Ok(None)
}

fn read_rust_toolchain(path: &Path) -> Result<Option<String>, anyhow::Error> {
    match fs::read_to_string(path) {
        Ok(file) => Ok(Some(file)),
        Err(e) => {
            if std::io::ErrorKind::NotFound != e.kind() {
                Err(anyhow::Error::from(e).context("Failed to read rust toolchain file."))
            } else {
                Ok(None)
            }
        }
    }
}
