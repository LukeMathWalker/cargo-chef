//! Data model for caching scenarios.
//!
//! These types are intentionally small and dumb: they describe the workspace
//! structure and the kinds of modifications/expectations the test harness can
//! apply and verify.

/// Scenario definition used by the test builders to generate a workspace.
#[derive(Debug, Clone)]
pub(crate) struct Scenario {
    pub(crate) members: Vec<MemberSpec>,
    pub(crate) workspace_external_deps: Vec<ExternalDep>,
    pub(crate) workspace_local_deps: Vec<LocalDep>,
}

/// One workspace member and its dependencies.
#[derive(Debug, Clone)]
pub(crate) struct MemberSpec {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) local_deps: Vec<LocalDep>,
    pub(crate) external_deps: Vec<ExternalDep>,
    pub(crate) has_lib: bool,
    pub(crate) bins: Vec<String>,
    pub(crate) has_build_script: bool,
    pub(crate) workspace_deps: Vec<String>,
    pub(crate) renamed_local_deps: Vec<RenamedLocalDep>,
}

#[derive(Debug, Clone)]
pub(crate) struct RenamedLocalDep {
    pub(crate) alias: String,
    pub(crate) package: String,
    pub(crate) version: String,
}

/// Parsed external dependency with name + version.
#[derive(Debug, Clone)]
pub(crate) struct ExternalDep {
    pub(crate) name: String,
    pub(crate) version: String,
}

#[derive(Debug, Clone)]
pub(crate) struct LocalDep {
    pub(crate) name: String,
    pub(crate) version: Option<String>,
}

/// External dependency spec for a modification step.
pub(crate) struct ExternalDepSpec<'a> {
    pub(crate) name: &'a str,
    pub(crate) version: &'a str,
}

/// Mutation to apply before re-preparing the recipe.
pub(crate) enum Modification<'a> {
    ModifySource {
        member: &'a str,
    },
    ModifyBuildScript {
        member: &'a str,
    },
    AddExternalDep {
        member: &'a str,
        dep: ExternalDepSpec<'a>,
        section: DependencySection<'a>,
    },
    AddWorkspaceExternalDep {
        dep: ExternalDepSpec<'a>,
    },
    AddExternalDepFeature {
        member: &'a str,
        dep: &'a str,
        feature: &'a str,
    },
    BumpMemberVersion {
        member: &'a str,
        new_version: &'a str,
    },
    BumpLocalDepVersion {
        member: &'a str,
        dep: &'a str,
        package: Option<&'a str>,
        new_version: &'a str,
    },
    BumpWorkspaceLocalDepVersion {
        dep: &'a str,
        new_version: &'a str,
    },
    UpdateExternalDepInLockfile {
        member: &'a str,
        dep: &'a str,
        initial_version: &'a str,
        new_version: &'a str,
    },
}

/// Which dependency table should be mutated in a manifest.
#[derive(Debug, Clone, Copy)]
pub(crate) enum DependencySection<'a> {
    Dependencies,
    DevDependencies,
    BuildDependencies,
    TargetDependencies { cfg: &'a str },
}

/// Expected outcome for a scenario run, validated in [`Scenario::run`].
pub(crate) enum Expectation {
    RecipeChanged,
    RecipeUnchanged,
    ExternalDepsFresh,
    ExternalDepsRebuilt,
    BuildSucceeds,
}

/// Options controlling how [`Scenario::run`] executes `prepare`/`build`.
#[derive(Debug, Clone, Default)]
pub(crate) struct RunOptions {
    pub(crate) prepare_bin: Option<String>,
    pub(crate) build_bin: Option<String>,
    pub(crate) cook_package: Option<String>,
    pub(crate) build_package: Option<String>,
    pub(crate) cook_profile: Option<String>,
    pub(crate) build_profile: Option<String>,
    pub(crate) target: Option<String>,
    pub(crate) target_dir: Option<String>,
}

impl RunOptions {
    /// Restrict both prepare and build to the same binary target.
    pub(crate) fn for_bin(bin: &str) -> Self {
        Self {
            prepare_bin: Some(bin.to_string()),
            build_bin: Some(bin.to_string()),
            cook_package: None,
            build_package: None,
            cook_profile: None,
            build_profile: None,
            target: None,
            target_dir: None,
        }
    }

    /// Restrict both cook and build to the same package target.
    pub(crate) fn for_package(package: &str) -> Self {
        Self {
            prepare_bin: None,
            build_bin: None,
            cook_package: Some(package.to_string()),
            build_package: Some(package.to_string()),
            cook_profile: None,
            build_profile: None,
            target: None,
            target_dir: None,
        }
    }
}

/// Builder for one [`MemberSpec`] in a [`Scenario`].
#[derive(Debug)]
pub(crate) struct Member {
    spec: MemberSpec,
}

impl Scenario {
    /// Start defining a workspace scenario.
    pub(crate) fn workspace() -> Self {
        Self {
            members: Vec::new(),
            workspace_external_deps: Vec::new(),
            workspace_local_deps: Vec::new(),
        }
    }

    /// Add a configured member to this scenario.
    pub(crate) fn member(mut self, member: Member) -> Self {
        self.members.push(member.spec);
        self
    }

    /// Add an external dependency to `[workspace.dependencies]`.
    pub(crate) fn workspace_external_dep(mut self, dep: impl Into<ExternalDep>) -> Self {
        self.workspace_external_deps.push(dep.into());
        self
    }

    /// Add a local dependency to `[workspace.dependencies]` (`name` or `name@version`).
    pub(crate) fn workspace_local_dep(mut self, dep: impl Into<LocalDep>) -> Self {
        self.workspace_local_deps.push(dep.into());
        self
    }
}

impl Member {
    /// Create a library crate member.
    pub(crate) fn lib(name: &str) -> Self {
        Self {
            spec: MemberSpec {
                name: name.to_string(),
                version: "0.1.0".to_string(),
                local_deps: Vec::new(),
                external_deps: Vec::new(),
                has_lib: true,
                bins: Vec::new(),
                has_build_script: false,
                workspace_deps: Vec::new(),
                renamed_local_deps: Vec::new(),
            },
        }
    }

    /// Create a binary crate member with one binary target named `name`.
    pub(crate) fn bin(name: &str) -> Self {
        Self {
            spec: MemberSpec {
                name: name.to_string(),
                version: "0.1.0".to_string(),
                local_deps: Vec::new(),
                external_deps: Vec::new(),
                has_lib: false,
                bins: vec![name.to_string()],
                has_build_script: false,
                workspace_deps: Vec::new(),
                renamed_local_deps: Vec::new(),
            },
        }
    }

    /// Override package version.
    pub(crate) fn version(mut self, version: &str) -> Self {
        self.spec.version = version.to_string();
        self
    }

    /// Add a local path dependency (`name` or `name@version`).
    pub(crate) fn local_dep(mut self, dep: impl Into<LocalDep>) -> Self {
        self.spec.local_deps.push(dep.into());
        self
    }

    /// Add an external dependency (`name@version`).
    pub(crate) fn external_dep(mut self, dep: impl Into<ExternalDep>) -> Self {
        self.spec.external_deps.push(dep.into());
        self
    }

    /// Add a binary target by name.
    pub(crate) fn with_bin(mut self, bin: &str) -> Self {
        self.spec.bins.push(bin.to_string());
        self
    }

    /// Enable a `build.rs` script for this member.
    pub(crate) fn with_build_script(mut self) -> Self {
        self.spec.has_build_script = true;
        self
    }

    /// Add a dependency inherited from `[workspace.dependencies]`.
    pub(crate) fn workspace_dep(mut self, dep: &str) -> Self {
        self.spec.workspace_deps.push(dep.to_string());
        self
    }

    /// Add a renamed local dependency entry.
    pub(crate) fn renamed_local_dep(mut self, alias: &str, package: &str, version: &str) -> Self {
        self.spec.renamed_local_deps.push(RenamedLocalDep {
            alias: alias.to_string(),
            package: package.to_string(),
            version: version.to_string(),
        });
        self
    }
}

impl ExternalDep {
    pub(crate) fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
        }
    }
}

impl LocalDep {
    pub(crate) fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            version: None,
        }
    }

    pub(crate) fn with_version(mut self, version: &str) -> Self {
        self.version = Some(version.to_string());
        self
    }
}

impl From<&str> for ExternalDep {
    fn from(value: &str) -> Self {
        let (name, version) = value
            .split_once('@')
            .expect("external dep must be in name@version format");
        Self::new(name, version)
    }
}

impl From<(&str, &str)> for ExternalDep {
    fn from((name, version): (&str, &str)) -> Self {
        Self::new(name, version)
    }
}

impl From<&str> for LocalDep {
    fn from(value: &str) -> Self {
        match value.split_once('@') {
            Some((name, version)) => Self::new(name).with_version(version),
            None => Self::new(value),
        }
    }
}

impl From<(&str, &str)> for LocalDep {
    fn from((name, version): (&str, &str)) -> Self {
        Self::new(name).with_version(version)
    }
}
