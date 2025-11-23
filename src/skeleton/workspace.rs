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

    let relevant_members = compute_transitive_deps(member, &members_to_members_graph);
    let relevant_dependencies = compute_transitive_deps(member, &members_to_dependencies_graph);

    // Remove all workspace members from the root manifest
    ignore_all_members_except(manifests, &metadata, member);

    // Retain only the manifests of the relevant workspace member
    manifests.retain(|manifest| {
        extract_pkg_name(&manifest.contents).is_none_or(|name| relevant_members.contains(&name))
    });

    // Filter lockfile to keep only relevant dependencies
    if let Some(lockfile) = lock_file {
        filter_lockfile(lockfile, &workspace_members, &relevant_members)?;
        filter_lockfile(lockfile, &workspace_dependencies, &relevant_dependencies)?;
    };

    Ok(())
}

/// Builds a dependency graph from a list of parsed Cargo manifests.
///
/// For each manifest, this function collects all dependencies
/// under `[dependencies]` and `[dev-dependencies]` that are
/// also present in the provided `target_deps` set.
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

/// Compute all transitive dependencies of the given target member.
fn compute_transitive_deps(
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

/// Filter lockfile to keep only relevant dependencies
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

/// Extract the crate name from contents
fn extract_pkg_name(contents: &Value) -> Option<String> {
    contents
        .get("package")?
        .get("name")?
        .as_str()
        .map(ToOwned::to_owned)
}

/// If the top-level `Cargo.toml` has a `members` field, replace it with
/// a list consisting of just the path to the package.
///
/// Also deletes the `default-members` field because it does not play nicely
/// with a modified `members` field and has no effect on cooking the final recipe.
fn ignore_all_members_except(manifests: &mut [ParsedManifest], metadata: &Metadata, member: &str) {
    let workspace_toml = manifests
        .iter_mut()
        .find(|manifest| manifest.relative_path == std::path::PathBuf::from("Cargo.toml"));

    if let Some(workspace) = workspace_toml.and_then(|toml| toml.contents.get_mut("workspace")) {
        if let Some(members) = workspace.get_mut("members") {
            let workspace_root = &metadata.workspace_root;
            let workspace_packages = metadata.workspace_packages();

            if let Some(pkg) = workspace_packages
                .into_iter()
                .find(|pkg| pkg.name == member)
            {
                // Make this a relative path to the workspace, and remove the `Cargo.toml` child.
                let member_cargo_path = diff_paths(pkg.manifest_path.as_os_str(), workspace_root);
                let member_workspace_path = member_cargo_path
                    .as_ref()
                    .and_then(|path| path.parent())
                    .and_then(|dir| dir.to_str());

                if let Some(member_path) = member_workspace_path {
                    *members =
                        toml::Value::Array(vec![toml::Value::String(member_path.to_string())]);
                }
            }
        }
        if let Some(workspace) = workspace.as_table_mut() {
            workspace.remove("default-members");
        }
    }
}
