use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use cargo_metadata::Metadata;
use pathdiff::diff_paths;
use toml::Value;

use crate::skeleton::ParsedManifest;

pub(super) fn reduce_workspace_by_member(
    metadata: &Metadata,
    manifests: &mut Vec<ParsedManifest>,
    lock_file: &mut Option<Value>,
    member: &str,
) -> Result<()> {
    let package_name = resolve_to_package_name(metadata, member);

    let workspace_members = manifests
        .iter()
        .filter_map(|m| extract_pkg_name(&m.contents))
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

    let relevant_members =
        compute_transitive_dependencies(&package_name, &members_to_members_graph);
    let relevant_dependencies =
        compute_transitive_dependencies(&package_name, &members_to_dependencies_graph);

    update_workspace_members(manifests, metadata, &relevant_members);

    manifests.retain(|manifest| {
        extract_pkg_name(&manifest.contents).is_none_or(|name| relevant_members.contains(&name))
    });

    if let Some(lockfile) = lock_file {
        filter_lockfile(lockfile, &workspace_members, &relevant_members)?;
        filter_lockfile(lockfile, &workspace_dependencies, &relevant_dependencies)?;
    };

    Ok(())
}

fn build_dependency_graph(
    manifests: &[ParsedManifest],
    target_deps: &HashSet<String>,
) -> HashMap<String, HashSet<String>> {
    let mut graph = HashMap::new();

    for manifest in manifests {
        if let Some(pkg_name) = extract_pkg_name(&manifest.contents) {
            let mut deps = HashSet::new();
            for key in ["dependencies", "dev-dependencies"] {
                if let Some(table) = manifest.contents.get(key).and_then(|v| v.as_table()) {
                    for (dep_name, _) in table {
                        if target_deps.contains(dep_name.as_str()) {
                            deps.insert(dep_name.to_string());
                        }
                    }
                }
            }
            graph.insert(pkg_name.clone(), deps);
        }
    }

    graph
}

/// Starting from `target`, walk the dependency graph and collect all reachable nodes.
fn compute_transitive_dependencies(
    target: &str,
    deps: &HashMap<String, HashSet<String>>,
) -> HashSet<String> {
    let mut keep = HashSet::new();
    let mut stack = vec![target.to_string()];

    while let Some(member) = stack.pop() {
        if keep.insert(member.clone()) {
            if let Some(children) = deps.get(&member) {
                stack.extend(children.iter().cloned());
            }
        }
    }

    keep
}

/// Filter the lockfile to only keep packages needed for the target build.
fn filter_lockfile(
    lock_file: &mut cargo_manifest::Value,
    all: &HashSet<String>,
    relevant: &HashSet<String>,
) -> Result<()> {
    let cargo_manifest::Value::Table(lock_table) = lock_file else {
        return Ok(());
    };

    let packages = match lock_table.get_mut("package").and_then(|v| v.as_array_mut()) {
        Some(arr) => arr,
        None => return Ok(()),
    };

    packages.retain(|pkg| {
        pkg.as_table()
            .and_then(|t| t.get("name").and_then(|v| v.as_str()))
            .map_or(true, |name| !all.contains(name) || relevant.contains(name))
    });

    Ok(())
}

fn extract_pkg_name(contents: &Value) -> Option<String> {
    contents
        .get("package")?
        .get("name")?
        .as_str()
        .map(ToOwned::to_owned)
}

/// If `member` is a binary name, find the package containing it. Otherwise return as-is.
fn resolve_to_package_name(metadata: &Metadata, member: &str) -> String {
    let workspace_packages = metadata.workspace_packages();

    if workspace_packages.iter().any(|pkg| pkg.name == member) {
        return member.to_string();
    }

    for pkg in workspace_packages {
        for target in &pkg.targets {
            if target.is_bin() && target.name == member {
                return pkg.name.clone();
            }
        }
    }

    member.to_string()
}

/// Rewrite the root `Cargo.toml` so that `[workspace] members` only lists packages
/// in `relevant_members`. Also removes `default-members` to avoid conflicts.
fn update_workspace_members(
    manifests: &mut [ParsedManifest],
    metadata: &Metadata,
    relevant_members: &HashSet<String>,
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
            .filter(|pkg| relevant_members.contains(&pkg.name))
            .filter_map(|pkg| {
                diff_paths(&pkg.manifest_path, workspace_root)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_transitive_dependencies() {
        let mut graph = HashMap::new();
        graph.insert("app".to_string(), HashSet::from(["core".to_string()]));
        graph.insert("core".to_string(), HashSet::from(["utils".to_string()]));
        graph.insert("utils".to_string(), HashSet::new());

        let result = compute_transitive_dependencies("app", &graph);
        assert!(result.contains("app"));
        assert!(result.contains("core"));
        assert!(result.contains("utils"));
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_filter_lockfile() {
        let mut lockfile: toml::Value = toml::from_str(
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

        let all_members = HashSet::from(["app".to_string(), "lib".to_string()]);
        let relevant = HashSet::from(["app".to_string()]);

        filter_lockfile(&mut lockfile, &all_members, &relevant).unwrap();

        let packages = lockfile.get("package").unwrap().as_array().unwrap();
        let names: Vec<_> = packages
            .iter()
            .filter_map(|p| p.get("name")?.as_str())
            .collect();

        // "lib" was filtered out, "app" and "serde" remain
        assert_eq!(names, vec!["app", "serde"]);
    }

    #[test]
    fn test_extract_pkg_name() {
        let toml: Value = toml::from_str(
            r#"
[package]
name = "my-crate"
version = "0.1.0"
"#,
        )
        .unwrap();

        assert_eq!(extract_pkg_name(&toml), Some("my-crate".to_string()));
    }
}
