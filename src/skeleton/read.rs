//! Logic to read all the files required to build a caching layer for a project.
use super::ParsedManifest;
use cargo_manifest::Manifest;
use cargo_metadata::Metadata;
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
    metadata: Metadata,
) -> Result<Vec<ParsedManifest>, anyhow::Error> {
    fn try_read_manifest(path: &Path) -> anyhow::Result<Manifest> {
        let contents: String = fs::read_to_string(path)?;

        Ok(cargo_manifest::Manifest::from_str(&contents)?)
    }

    let mut manifest_paths = metadata
        .packages
        .iter()
        .filter_map(|p| {
            if p.source.is_none() {
                Some(p.manifest_path.to_path_buf())
            } else {
                None
            }
        })
        .collect::<BTreeSet<_>>();
    manifest_paths.insert(metadata.workspace_root.join("Cargo.toml"));

    let mut manifests = BTreeMap::<PathBuf, Manifest>::new();
    for absolute_path in manifest_paths {
        let parsed = try_read_manifest(absolute_path.as_std_path())?;

        // it's possible that a path dependency could reference a different Workspace Root
        if let Some(workspace) = parsed.package.as_ref().and_then(|p| p.workspace.as_ref()) {
            let workspace_path = absolute_path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Unable to get parent of {}", absolute_path))?
                .join(workspace)
                .join("Cargo.toml")
                .canonicalize()?;
            if !manifests.contains_key(&workspace_path) {
                manifests.insert(workspace_path.clone(), try_read_manifest(&workspace_path)?);
            }
        }

        manifests.insert(absolute_path.into_std_path_buf(), parsed);
    }

    manifests
        .into_iter()
        .map(|(absolute_path, mut parsed)| {
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

            let relative_path =
                pathdiff::diff_paths(&absolute_path, base_path).ok_or_else(|| {
                    anyhow::anyhow!(
                        "Failed to compute relative path of manifest {:?}",
                        &absolute_path
                    )
                })?;
            Ok(ParsedManifest {
                relative_path,
                contents: intermediate,
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
