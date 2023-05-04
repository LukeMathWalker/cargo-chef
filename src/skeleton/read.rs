//! Logic to read all the files required to build a caching layer for a project.
use super::ParsedManifest;
use crate::skeleton::target::{Target, TargetKind};
use cargo_metadata::{Metadata, Package};
use std::fs;
use std::path::Path;
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
    metadata: Metadata,
) -> Result<Vec<ParsedManifest>, anyhow::Error> {
    let mut packages = metadata
        .workspace_packages()
        .iter()
        .copied()
        .chain(metadata.root_package())
        .map(|p| (Some(p), p.manifest_path.clone().into_std_path_buf()))
        .collect::<Vec<_>>();

    if metadata.root_package().is_none() {
        // At the root, there might be a Cargo.toml manifest with a [workspace] section.
        // However, if this root manifest doesn't contain [package], it is not considered a package
        // by cargo metadata. Therefore, we have to add it manually.
        // Workspaces currently cannot be nested, so this should only happen at the root.
        packages.push((None, base_path.as_ref().join("Cargo.toml")));
    }

    packages.sort_by(|a, b| a.1.cmp(&b.1));
    packages.dedup();

    let mut manifests = vec![];
    for (package, absolute_path) in packages {
        let contents = fs::read_to_string(&absolute_path)?;

        let mut parsed = cargo_manifest::Manifest::from_str(&contents)?;
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
        let mut targets = package.map(|p| gather_targets(p)).unwrap_or_default();
        targets.sort_by(|a, b| a.path.cmp(&b.path));

        manifests.push(ParsedManifest {
            relative_path,
            contents: intermediate,
            targets,
        });
    }

    Ok(manifests)
}
// I started implementing resolving targets through `cargo metadata`, but I wonder if it makes sense. We currently recreate
fn gather_targets(package: &Package) -> Vec<Target> {
    let manifest = package.manifest_path.clone().into_std_path_buf();
    let root_dir = manifest.parent().unwrap();
    package
        .targets
        .iter()
        .filter_map(|target| {
            let relative_path = target
                .src_path
                .strip_prefix(root_dir)
                .unwrap()
                .to_path_buf()
                .into_std_path_buf();
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
                        .find(|t| t.as_str() == "proc-macro")
                        .is_some(),
                }
            };

            Some(Target {
                path: relative_path,
                kind,
                name: target.name.clone(),
            })
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
