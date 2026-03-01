//! Workspace filtering for `--bin` builds.
//!
//! Filters unrequired workspace members from manifests and lockfile.
//!
//! Limitation: external/transitive deps are NOT filtered (e.g. `tokio-macros`
//! stays even if `tokio` is removed).

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use cargo_metadata::Metadata;
use pathdiff::diff_paths;
use toml::Value;

use crate::skeleton::ParsedManifest;

pub(super) fn filter_workspace_for_target(
    metadata: &Metadata,
    manifests: &mut Vec<ParsedManifest>,
    lock_file: &mut Option<Value>,
    target_name: &str,
) -> Result<()> {
    let target_member = resolve_binary_to_package_name(metadata, target_name);

    let workspace_members = manifests
        .iter()
        .filter_map(|m| extract_package_name(&m.contents))
        .collect();

    let root_manifest = manifests
        .iter_mut()
        .find(|m| m.relative_path.to_str() == Some("Cargo.toml"))
        .context("no root manifest found")?;

    let root_manifest_contents = root_manifest
        .contents
        .get("workspace")
        .context("get workspace")?;

    let workspace_dependencies = match root_manifest_contents.get("dependencies") {
        Some(v) => {
            let table = v.as_table().context("dependencies must be a table")?;
            table.iter().map(|(name, _)| name.to_string()).collect()
        }
        None => HashSet::new(),
    };

    let members_to_members_graph = build_dependency_graph(&manifests, &workspace_members);
    let members_to_dependencies_graph = build_dependency_graph(&manifests, &workspace_dependencies);

    let required_members = collect_required_dependencies(&target_member, &members_to_members_graph);
    let required_dependencies =
        collect_required_dependencies(&target_member, &members_to_dependencies_graph);

    filter_root_manifest(manifests, metadata, &required_members);

    filter_member_manifests(manifests, &required_members);

    filter_lockfile(
        lock_file,
        &workspace_members,
        &workspace_dependencies,
        &required_members,
        &required_dependencies,
    )?;

    Ok(())
}

/// Builds a package to dependencies map, only including dependencies present in `target_dependencies`.
fn build_dependency_graph(
    manifests: &[ParsedManifest],
    target_dependencies: &HashSet<String>,
) -> HashMap<String, HashSet<String>> {
    let mut graph = HashMap::new();

    for manifest in manifests {
        if let Some(package_name) = extract_package_name(&manifest.contents) {
            let mut dependencies = HashSet::new();
            for key in ["dependencies", "dev-dependencies"] {
                if let Some(table) = manifest.contents.get(key).and_then(|v| v.as_table()) {
                    for (name, _) in table {
                        if target_dependencies.contains(name.as_str()) {
                            dependencies.insert(name.to_string());
                        }
                    }
                }
            }
            graph.insert(package_name.clone(), dependencies);
        }
    }

    graph
}

/// Returns all transitive dependencies of the given target member.
fn collect_required_dependencies(
    target: &str,
    dependencies: &HashMap<String, HashSet<String>>,
) -> HashSet<String> {
    let mut keep = HashSet::new();
    let mut stack = vec![target.to_string()];

    while let Some(member) = stack.pop() {
        if keep.insert(member.clone()) {
            if let Some(children) = dependencies.get(&member) {
                stack.extend(children.iter().cloned());
            }
        }
    }

    keep
}

/// Filters the root manifest to remove unrequired members.
/// Also removes `default-members` if present.
fn filter_root_manifest(
    manifests: &mut [ParsedManifest],
    metadata: &Metadata,
    required_members: &HashSet<String>,
) {
    let workspace_toml = manifests
        .iter_mut()
        .find(|m| m.relative_path == std::path::PathBuf::from("Cargo.toml"));

    let Some(workspace) = workspace_toml.and_then(|toml| toml.contents.get_mut("workspace")) else {
        return;
    };

    if let Some(members) = workspace.get_mut("members") {
        let workspace_root = &metadata.workspace_root;
        let member_paths: Vec<toml::Value> = metadata
            .workspace_packages()
            .iter()
            .filter(|package| required_members.contains(&package.name))
            .filter_map(|package| {
                diff_paths(&package.manifest_path, workspace_root)
                    .and_then(|p| p.parent().map(|d| d.to_path_buf()))
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
}

/// Filters the member manifests to remove unrequired members.
fn filter_member_manifests(
    manifests: &mut Vec<ParsedManifest>,
    required_members: &HashSet<String>,
) {
    manifests.retain(|manifest| {
        extract_package_name(&manifest.contents).is_none_or(|name| required_members.contains(&name))
    });
}

/// Filters the lockfile to remove unrequired workspace packages.
fn filter_lockfile(
    lock_file: &mut Option<Value>,
    workspace_members: &HashSet<String>,
    workspace_dependencies: &HashSet<String>,
    required_members: &HashSet<String>,
    required_dependencies: &HashSet<String>,
) -> Result<()> {
    let Some(lock_file) = lock_file else {
        return Ok(());
    };

    let all_workspace: HashSet<String> = workspace_members
        .union(workspace_dependencies)
        .cloned()
        .collect();
    let all_required: HashSet<String> = required_members
        .union(required_dependencies)
        .cloned()
        .collect();

    let cargo_manifest::Value::Table(lock_table) = lock_file else {
        return Ok(());
    };

    let packages = match lock_table.get_mut("package").and_then(|v| v.as_array_mut()) {
        Some(arr) => arr,
        None => return Ok(()),
    };

    // Limitation: transitive deps of removed packages are NOT filtered
    // (e.g. tokio-macros stays even if tokio is removed).
    packages.retain(|package| {
        let Some(name) = package
            .as_table()
            .and_then(|t| t.get("name"))
            .and_then(|v| v.as_str())
        else {
            return true;
        };
        all_required.contains(name) || !all_workspace.contains(name)
    });

    Ok(())
}

// The binary name passed via --bin is not necessarily the package name.
// Look up which package contains this binary target.
fn resolve_binary_to_package_name(metadata: &Metadata, target_name: &str) -> String {
    let workspace_packages = metadata.workspace_packages();

    if workspace_packages
        .iter()
        .any(|package| package.name == target_name)
    {
        return target_name.to_string();
    }

    for package in workspace_packages {
        for target in &package.targets {
            if target.is_bin() && target.name == target_name {
                return package.name.clone();
            }
        }
    }

    target_name.to_string()
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
    fn test_collect_required_dependencies() {
        let mut graph = HashMap::new();
        graph.insert("app".to_string(), HashSet::from(["core".to_string()]));
        graph.insert("core".to_string(), HashSet::from(["utils".to_string()]));
        graph.insert("utils".to_string(), HashSet::new());

        let result = collect_required_dependencies("app", &graph);
        assert!(result.contains("app"));
        assert!(result.contains("core"));
        assert!(result.contains("utils"));
        assert_eq!(result.len(), 3);
    }

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
        let workspace_members = HashSet::from(["app".to_string(), "lib".to_string()]);
        let required_members = HashSet::from(["app".to_string()]);

        filter_lockfile(
            &mut lockfile,
            &workspace_members,
            &HashSet::new(),
            &required_members,
            &HashSet::new(),
        )
        .unwrap();

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
