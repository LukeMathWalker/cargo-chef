//! Logic to read all the files required to build a caching layer for a project.
use super::ParsedManifest;
use anyhow::Context;
use globwalk::{GlobWalkerBuilder, WalkError};
use std::fs;
use std::path::Path;
use std::str::FromStr;

pub(super) fn config<P: AsRef<Path>>(base_path: &P) -> Result<Option<String>, anyhow::Error> {
    // Given that we run primarily in Docker, assume to find config.toml at root level.
    match fs::read_to_string(base_path.as_ref().join(".cargo/config.toml")) {
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

fn vendored_directory(config_contents: Option<&str>) -> Option<String> {
    let contents = config_contents.and_then(|contents| contents.parse::<toml::Value>().ok())?;
    let source = contents.get("source")?;
    let crates_io = source.get("crates-io")?;
    let vendored_field_suffix = crates_io
        .get("replace-with")
        .and_then(|value| value.as_str())?;
    let vendored_sources = source.get(vendored_field_suffix)?;
    Some(vendored_sources.get("directory")?.as_str()?.to_owned())
}

pub(super) fn manifests<P: AsRef<Path>>(
    base_path: &P,
    config_contents: Option<&str>,
) -> Result<Vec<ParsedManifest>, anyhow::Error> {
    let vendored_path = vendored_directory(config_contents);
    let builder = if let Some(path) = vendored_path {
        let exclude_vendored_sources = "!".to_string() + &path;
        GlobWalkerBuilder::from_patterns(&base_path, &["/**/Cargo.toml", &exclude_vendored_sources])
    } else {
        GlobWalkerBuilder::new(&base_path, "/**/Cargo.toml")
    };
    let walker = builder
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
                println!("{:?}", intermediate);

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

                let relative_path =
                    pathdiff::diff_paths(&absolute_path, &base_path).ok_or_else(|| {
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
            Err(e) => match handle_walk_error(e) {
                ErrorStrategy::Ignore => {}
                ErrorStrategy::Crash(e) => {
                    return Err(e.into());
                }
            },
        }
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
