//! Logic to read all the files required to build a caching layer for a project.
use super::ParsedManifest;
use cargo_metadata::Metadata;
use std::collections::HashSet;
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
    let mut metadata_paths = metadata
        .workspace_packages()
        .into_iter()
        .map(|package| package.manifest_path.clone().into_std_path_buf())
        .collect::<HashSet<_>>();
    metadata_paths.insert(base_path.as_ref().join("Cargo.toml"));
    let mut manifest_paths: Vec<PathBuf> = metadata_paths.into_iter().collect();
    manifest_paths.sort();

    let mut manifests = vec![];
    for absolute_path in manifest_paths {
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
        });
    }

    Ok(manifests)
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
