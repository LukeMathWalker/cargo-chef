//! Typed manifest mutations for caching test scenarios.

use std::path::Path;

use cargo_manifest::{Dependency, DependencyDetail, Manifest, MaybeInherited, Target, Workspace};

use crate::helpers::model::DependencySection;

pub(crate) fn add_external_dep_in_section(
    root: &Path,
    member: &str,
    name: &str,
    version: &str,
    section: &DependencySection<'_>,
) {
    let manifest_path = root.join(member).join("Cargo.toml");
    let mut manifest = load_manifest(&manifest_path);
    match section {
        DependencySection::Dependencies => {
            manifest
                .dependencies
                .get_or_insert_with(Default::default)
                .insert(name.to_string(), Dependency::Simple(version.to_string()));
        }
        DependencySection::DevDependencies => {
            manifest
                .dev_dependencies
                .get_or_insert_with(Default::default)
                .insert(name.to_string(), Dependency::Simple(version.to_string()));
        }
        DependencySection::BuildDependencies => {
            manifest
                .build_dependencies
                .get_or_insert_with(Default::default)
                .insert(name.to_string(), Dependency::Simple(version.to_string()));
        }
        DependencySection::TargetDependencies { cfg } => {
            let target = manifest
                .target
                .get_or_insert_with(Default::default)
                .entry(cfg.to_string())
                .or_insert_with(empty_target);
            target
                .dependencies
                .insert(name.to_string(), Dependency::Simple(version.to_string()));
        }
    }
    save_manifest(&manifest_path, &manifest);
}

pub(crate) fn add_workspace_external_dep(root: &Path, name: &str, version: &str) {
    let manifest_path = root.join("Cargo.toml");
    let mut manifest = load_manifest(&manifest_path);
    let workspace = manifest.workspace.get_or_insert_with(empty_workspace);
    workspace
        .dependencies
        .get_or_insert_with(Default::default)
        .insert(name.to_string(), Dependency::Simple(version.to_string()));
    save_manifest(&manifest_path, &manifest);
}

pub(crate) fn add_external_dep_feature(root: &Path, member: &str, dep: &str, feature: &str) {
    let manifest_path = root.join(member).join("Cargo.toml");
    let mut manifest = load_manifest(&manifest_path);
    let dependencies = manifest
        .dependencies
        .as_mut()
        .expect("expected [dependencies] table");
    let dep_entry = dependencies
        .get_mut(dep)
        .expect("expected dependency to exist");
    match dep_entry {
        Dependency::Simple(version) => {
            *dep_entry = Dependency::Detailed(DependencyDetail {
                version: Some(version.clone()),
                features: Some(vec![feature.to_string()]),
                ..DependencyDetail::default()
            });
        }
        Dependency::Detailed(detail) => {
            let features = detail.features.get_or_insert_with(Vec::new);
            if !features.iter().any(|existing| existing == feature) {
                features.push(feature.to_string());
            }
        }
        Dependency::Inherited(_) => panic!("expected non-inherited dependency"),
    }
    save_manifest(&manifest_path, &manifest);
}

pub(crate) fn bump_member_version(root: &Path, member: &str, new_version: &str) {
    let manifest_path = root.join(member).join("Cargo.toml");
    let mut manifest = load_manifest(&manifest_path);
    let package = manifest.package.as_mut().expect("expected [package] table");
    package.version = Some(MaybeInherited::Local(new_version.to_string()));
    save_manifest(&manifest_path, &manifest);
}

pub(crate) fn bump_local_dep_version(
    root: &Path,
    member: &str,
    dep: &str,
    package: Option<&str>,
    new_version: &str,
) {
    let manifest_path = root.join(member).join("Cargo.toml");
    let mut manifest = load_manifest(&manifest_path);
    let dependencies = manifest.dependencies.get_or_insert_with(Default::default);

    if let Some(package_name) = package {
        dependencies.remove(package_name);
    }

    dependencies.insert(
        dep.to_string(),
        Dependency::Detailed(DependencyDetail {
            path: Some(format!("../{}", package.unwrap_or(dep))),
            version: Some(new_version.to_string()),
            package: package.map(ToString::to_string),
            ..DependencyDetail::default()
        }),
    );
    save_manifest(&manifest_path, &manifest);

    // Keep the referenced local package version aligned with the new dependency
    // requirement to preserve a resolvable workspace lockfile.
    let package_name = package.unwrap_or(dep);
    bump_member_version(root, package_name, new_version);
}

pub(crate) fn bump_workspace_local_dep_version(root: &Path, dep: &str, new_version: &str) {
    let manifest_path = root.join("Cargo.toml");
    let mut manifest = load_manifest(&manifest_path);
    let workspace_dependencies = manifest
        .workspace
        .as_mut()
        .and_then(|workspace| workspace.dependencies.as_mut())
        .expect("expected [workspace.dependencies] table");
    let dep_entry = workspace_dependencies
        .get_mut(dep)
        .expect("expected dependency in [workspace.dependencies]");
    match dep_entry {
        Dependency::Simple(version) => *version = new_version.to_string(),
        Dependency::Detailed(detail) => detail.version = Some(new_version.to_string()),
        Dependency::Inherited(_) => panic!("unexpected inherited workspace dependency"),
    }
    save_manifest(&manifest_path, &manifest);

    bump_member_version(root, dep, new_version);
}

fn load_manifest(path: &Path) -> Manifest {
    let raw = std::fs::read_to_string(path).unwrap();
    Manifest::from_slice(raw.as_bytes()).expect("manifest should parse")
}

fn save_manifest(path: &Path, manifest: &Manifest) {
    std::fs::write(path, toml::to_string(manifest).unwrap()).unwrap();
}

fn empty_target() -> Target {
    Target {
        dependencies: Default::default(),
        dev_dependencies: Default::default(),
        build_dependencies: Default::default(),
    }
}

fn empty_workspace() -> Workspace {
    Workspace {
        members: Vec::new(),
        default_members: None,
        exclude: None,
        resolver: None,
        dependencies: None,
        package: None,
        metadata: None,
    }
}
