//! Workspace filtering for `--bin` builds.
//!
//! Filters unrequired workspace members and their dependencies from manifests and lockfile.

use std::collections::HashSet;

use anyhow::Result;
use guppy::graph::{BuildTargetId, DependencyDirection, PackageGraph};
use toml::Value;

use crate::skeleton::ParsedManifest;

pub(super) fn filter_workspace_for_target(
    graph: &PackageGraph,
    manifests: &mut Vec<ParsedManifest>,
    lock_file: &mut Option<Value>,
    target_name: &str,
) -> Result<()> {
    let workspace = graph.workspace();

    // Find the target package by name or binary target
    let target_pkg = match workspace.member_by_name(target_name) {
        Ok(pkg) => pkg,
        Err(_) => workspace
            .iter()
            .find(|pkg| {
                pkg.build_targets()
                    .any(|t| matches!(t.id(), BuildTargetId::Binary(name) if name == target_name))
            })
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No workspace package or binary target named '{}'",
                    target_name
                )
            })?,
    };

    // Get transitive dependencies of target package
    let resolved = graph
        .query_forward(std::iter::once(target_pkg.id()))?
        .resolve();

    // Collect workspace members required by the target (including transitive dependencies)
    let required_members: HashSet<String> = workspace
        .iter()
        .filter(|ws_pkg| resolved.contains(ws_pkg.id()).unwrap_or(false))
        .map(|ws_pkg| ws_pkg.name().to_string())
        .collect();

    // 1. Filter manifests: keep root workspace manifest + required members
    manifests.retain(|m| {
        extract_package_name(&m.contents).is_none_or(|name| required_members.contains(&name))
    });

    // 2. Update [workspace.members] in root manifest and remove default-members
    filter_root_manifest(manifests, graph, &required_members)?;

    // 3. Collect ALL (name, version) required member pairs (workspace + external)
    let closure_packages: HashSet<(String, String)> = resolved
        .packages(DependencyDirection::Forward)
        .map(|pkg| (pkg.name().to_string(), pkg.version().to_string()))
        .collect();

    // 4. Filter lockfile: keep only required packages
    if let Some(lockfile) = lock_file {
        filter_lockfile(lockfile, &closure_packages)?;
    }

    Ok(())
}

/// Filters `[workspace] members` to only include required packages.
/// Also removes `default-members` if present.
fn filter_root_manifest(
    manifests: &mut [ParsedManifest],
    graph: &PackageGraph,
    required_members: &HashSet<String>,
) -> Result<()> {
    let workspace_toml = manifests
        .iter_mut()
        .find(|m| m.relative_path == std::path::PathBuf::from("Cargo.toml"));

    let Some(workspace) = workspace_toml.and_then(|toml| toml.contents.get_mut("workspace")) else {
        return Ok(());
    };

    if let Some(members) = workspace.get_mut("members") {
        let workspace_root = graph.workspace().root();
        let member_paths: Vec<toml::Value> = graph
            .workspace()
            .iter()
            .filter(|pkg| required_members.contains(pkg.name()))
            .filter_map(|pkg| {
                let manifest_path = pkg.manifest_path();
                manifest_path
                    .parent()
                    .and_then(|p| pathdiff::diff_paths(p, workspace_root))
                    .and_then(|d| d.to_str().map(|s| toml::Value::String(s.to_string())))
            })
            .collect();

        if !member_paths.is_empty() {
            *members = toml::Value::Array(member_paths);
        }
    }

    if let Some(workspace) = workspace.as_table_mut() {
        workspace.remove("default-members");
    }

    Ok(())
}

/// Filters the lockfile to keep only required packages.
/// Matches packages by (name, version) pairs.
fn filter_lockfile(
    lock_file: &mut Value,
    required_packages: &HashSet<(String, String)>,
) -> Result<()> {
    let cargo_manifest::Value::Table(lock_table) = lock_file else {
        return Ok(());
    };

    let packages = match lock_table.get_mut("package").and_then(|v| v.as_array_mut()) {
        Some(arr) => arr,
        None => return Ok(()),
    };

    packages.retain(|package| {
        let Some(pkg_table) = package.as_table() else {
            return true;
        };

        let name = pkg_table.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let version = pkg_table
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        required_packages.contains(&(name.to_string(), version.to_string()))
    });

    Ok(())
}

fn extract_package_name(contents: &Value) -> Option<String> {
    contents
        .get("package")?
        .get("name")?
        .as_str()
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_lockfile() {
        let lockfile: toml::Value = toml::from_str(
            r#"
[[package]]
name = "app"
version = "0.1.0"

[[package]]
name = "lib"
version = "0.1.0"

[[package]]
name = "serde"
version = "1.0.0"
"#,
        )
        .unwrap();

        let mut lockfile = Some(lockfile);
        let required_members = HashSet::from([
            ("app".to_string(), "0.1.0".to_string()),
            ("serde".to_string(), "1.0.0".to_string()),
        ]);

        filter_lockfile(lockfile.as_mut().unwrap(), &required_members).unwrap();

        let packages = lockfile
            .as_ref()
            .unwrap()
            .get("package")
            .unwrap()
            .as_array()
            .unwrap()
            .clone();
        let names: Vec<_> = packages
            .iter()
            .filter_map(|p| p.get("name")?.as_str())
            .collect();

        // "lib" was filtered out, "app" and "serde" remain
        assert_eq!(names, vec!["app", "serde"]);
    }

    #[test]
    fn test_extract_package_name() {
        let toml: Value = toml::from_str(
            r#"
[package]
name = "my-crate"
version = "0.1.0"
"#,
        )
        .unwrap();

        assert_eq!(extract_package_name(&toml), Some("my-crate".to_string()));
    }
}
