//! Project generation and mutation logic for scenarios.
//!
//! Responsible for writing workspace manifests and stub source files based on
//! a `Scenario`, plus applying targeted modifications.

use assert_fs::TempDir;
use std::path::Path;

use crate::helpers::fs::write_file;
use crate::helpers::model::{ExternalDep, LocalDep, MemberSpec, Scenario};

impl Scenario {
    pub(crate) fn build_project(&self) -> TempDir {
        let root = TempDir::new().unwrap();
        write_file(
            root.path().join("Cargo.toml"),
            &workspace_manifest(
                self.members
                    .iter()
                    .map(|m| m.name.as_str())
                    .collect::<Vec<_>>(),
                &self.workspace_external_deps,
                &self.workspace_local_deps,
            ),
        );
        for member in &self.members {
            let member_root = root.path().join(&member.name);
            write_file(member_root.join("Cargo.toml"), &member_manifest(member));
            if member.has_lib {
                write_file(
                    member_root.join("src/lib.rs"),
                    &member_source(&member.name, 0),
                );
            }
            for bin in &member.bins {
                write_file(
                    member_root.join("src/bin").join(format!("{bin}.rs")),
                    "fn main() {}\n",
                );
            }
            if member.has_build_script {
                write_file(member_root.join("build.rs"), "fn main() {}\n");
            }
        }
        root
    }

    pub(crate) fn modify_source(&self, root: &Path, member: &str) {
        let member_root = root.join(member);
        let spec = self
            .members
            .iter()
            .find(|m| m.name == member)
            .expect("unknown member");
        if spec.has_lib {
            write_file(member_root.join("src/lib.rs"), &member_source(member, 1));
        } else {
            let bin = spec
                .bins
                .first()
                .expect("bin member must have at least one bin target");
            write_file(
                member_root.join("src/bin").join(format!("{bin}.rs")),
                "fn main() { let _ = 1 + 1; }\n",
            );
        }
    }

    pub(crate) fn modify_build_script(&self, root: &Path, member: &str) {
        let member_root = root.join(member);
        write_file(
            member_root.join("build.rs"),
            "fn main() { println!(\"cargo:rustc-cfg=changed\"); }\n",
        );
    }

    pub(crate) fn external_deps(&self, member: &str) -> Vec<ExternalDep> {
        let spec = self
            .members
            .iter()
            .find(|m| m.name == member)
            .expect("unknown member");
        let mut deps = spec.external_deps.to_vec();
        for workspace_dep in &spec.workspace_deps {
            if self
                .workspace_local_deps
                .iter()
                .any(|dep| dep.name == *workspace_dep)
            {
                continue;
            }
            if let Some(raw) = self
                .workspace_external_deps
                .iter()
                .find(|dep| dep.name == *workspace_dep)
            {
                deps.push(raw.clone());
            }
        }
        deps
    }
}

fn workspace_manifest(
    members: Vec<&str>,
    workspace_external_deps: &[ExternalDep],
    workspace_local_deps: &[LocalDep],
) -> String {
    format!(
        "[workspace]\nresolver = \"3\"\nmembers = [{}]\n{}",
        members
            .iter()
            .map(|m| format!("\"{m}\""))
            .collect::<Vec<_>>()
            .join(", "),
        workspace_dependencies_manifest(workspace_external_deps, workspace_local_deps),
    )
}

fn member_manifest(member: &MemberSpec) -> String {
    let mut manifest = String::new();
    manifest.push_str("[package]\n");
    manifest.push_str(&format!("name = \"{}\"\n", member.name));
    manifest.push_str(&format!("version = \"{}\"\n", member.version));
    if member.has_build_script {
        manifest.push_str("build = \"build.rs\"\n");
    }
    manifest.push_str("edition = \"2024\"\n\n[dependencies]\n");
    for dep in &member.external_deps {
        manifest.push_str(&format!("{} = \"{}\"\n", dep.name, dep.version));
    }
    for dep in &member.local_deps {
        match dep.version.as_deref() {
            Some(version) => {
                manifest.push_str(&format!(
                    "{} = {{ path = \"../{}\", version = \"{version}\" }}\n",
                    dep.name, dep.name
                ));
            }
            None => {
                manifest.push_str(&format!(
                    "{} = {{ path = \"../{}\" }}\n",
                    dep.name, dep.name
                ));
            }
        }
    }
    for dep in &member.workspace_deps {
        manifest.push_str(&format!("{dep} = {{ workspace = true }}\n"));
    }
    for dep in &member.renamed_local_deps {
        manifest.push_str(&format!(
            "{} = {{ package = \"{}\", path = \"../{}\", version = \"{}\" }}\n",
            dep.alias, dep.package, dep.package, dep.version
        ));
    }
    if !member.bins.is_empty() {
        for bin in &member.bins {
            manifest.push_str("\n[[bin]]\n");
            manifest.push_str(&format!("name = \"{bin}\"\n"));
            manifest.push_str(&format!("path = \"src/bin/{bin}.rs\"\n"));
        }
    }
    manifest
}

fn member_source(name: &str, adjustment: i32) -> String {
    let crate_name = name.replace('-', "_");
    format!(
        "pub fn id() -> i32 {{\n    42 + {adjustment}\n}}\n\npub fn name() -> &'static str {{\n    \"{crate_name}\"\n}}\n"
    )
}

fn workspace_dependencies_manifest(
    workspace_external_deps: &[ExternalDep],
    workspace_local_deps: &[LocalDep],
) -> String {
    if workspace_external_deps.is_empty() && workspace_local_deps.is_empty() {
        return String::new();
    }

    let mut out = String::from("\n[workspace.dependencies]\n");
    for dep in workspace_external_deps {
        out.push_str(&format!("{} = \"{}\"\n", dep.name, dep.version));
    }
    for dep in workspace_local_deps {
        let version = dep.version.as_deref().unwrap_or("0.1.0");
        out.push_str(&format!(
            "{} = {{ path = \"{}\", version = \"{version}\" }}\n",
            dep.name, dep.name
        ));
    }
    out
}
