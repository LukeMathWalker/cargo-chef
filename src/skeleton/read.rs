//! Logic to read all the files required to build a caching layer for a project.
use super::ParsedManifest;
use crate::skeleton::target::{Target, TargetKind};
use crate::RustToolchainFile;
use guppy::graph::{BuildTargetId, BuildTargetKind, PackageGraph, PackageMetadata};
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
    graph: &PackageGraph,
) -> Result<Vec<ParsedManifest>, anyhow::Error> {
    let workspace = graph.workspace();

    let mut packages: BTreeMap<PathBuf, BTreeSet<Target>> = workspace
        .iter()
        .map(|pkg| {
            (
                pkg.manifest_path().as_std_path().to_path_buf(),
                gather_targets(&pkg),
            )
        })
        .collect();

    // At the root, there might be a Cargo.toml manifest with a [workspace] section.
    // However, if this root manifest doesn't contain [package], it is not considered a package
    // by cargo metadata. Therefore, we have to add it manually.
    // Workspaces currently cannot be nested, so this should only happen at the root.
    let root_manifest = workspace.root().join("Cargo.toml");
    let root_is_package = packages.keys().any(|p| p == root_manifest.as_std_path());
    if !root_is_package {
        packages.insert(base_path.as_ref().join("Cargo.toml"), Default::default());
    }

    let mut manifests = vec![];
    for (absolute_path, targets) in packages {
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

        manifests.push(ParsedManifest {
            relative_path,
            contents: intermediate,
            targets: targets.into_iter().collect(),
        });
    }

    Ok(manifests)
}

fn gather_targets(package: &PackageMetadata) -> BTreeSet<Target> {
    let manifest_path = package.manifest_path().as_std_path();
    let root_dir = manifest_path.parent().unwrap();
    package
        .build_targets()
        .map(|target| {
            let relative_path =
                pathdiff::diff_paths(target.path().as_std_path(), root_dir).unwrap();
            let kind = match target.id() {
                BuildTargetId::Library => match target.kind() {
                    BuildTargetKind::ProcMacro => TargetKind::Lib {
                        is_proc_macro: true,
                    },
                    _ => TargetKind::Lib {
                        is_proc_macro: false,
                    },
                },
                BuildTargetId::Binary(_) => TargetKind::Bin,
                BuildTargetId::Test(_) => TargetKind::Test,
                BuildTargetId::Benchmark(_) => TargetKind::Bench,
                BuildTargetId::Example(_) => TargetKind::Example,
                BuildTargetId::BuildScript => TargetKind::BuildScript,
                // BuildTargetId is non_exhaustive
                other => panic!("unknown build target kind: {:?}", other),
            };

            Target {
                path: relative_path,
                kind,
                name: target.name().to_string(),
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
