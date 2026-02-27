//! Orchestrates the cache test steps and verifies expectations.
//!
//! This is the entrypoint used by the test cases: it runs the full
//! prepare/cook/modify/prepare/build flow and validates the requested outcome.

use assert_fs::TempDir;
use std::fs;

use crate::helpers::cargo::{
    cook_output_string, parse_compilation_output, run_cargo_build, run_cargo_update_precise,
    run_cook, run_generate_lockfile, run_prepare,
};
use crate::helpers::fs::sync_project_to_cook_dir;
use crate::helpers::manifest::{
    add_external_dep_feature, add_external_dep_in_section, add_workspace_external_dep,
    bump_local_dep_version, bump_member_version, bump_workspace_local_dep_version,
};
use crate::helpers::model::{Expectation, Modification, RunOptions, Scenario};

impl Scenario {
    /// Execute the full prepare/cook/modify/prepare/build flow and assert `expectation`.
    pub(crate) fn run(&self, modification: Modification<'_>, expectation: Expectation) {
        self.run_with_options(modification, expectation, RunOptions::default());
    }

    /// Execute the full flow with custom run options.
    pub(crate) fn run_with_options(
        &self,
        modification: Modification<'_>,
        expectation: Expectation,
        options: RunOptions,
    ) {
        let project = self.build_project();
        let recipe_path = project.path().join("recipe.json");
        let cook_dir = TempDir::new().unwrap();

        self.prepare_lockfile_for_first_prepare(project.path(), &modification);

        let original_recipe =
            self.run_initial_prepare_cook(project.path(), &recipe_path, &cook_dir, &options);

        let regenerate_lockfile = self.apply_modification(project.path(), &modification);

        if regenerate_lockfile {
            // Keep lockfile in sync with project state before the second recipe generation.
            run_generate_lockfile(project.path());
        }

        run_prepare(project.path(), &recipe_path, options.prepare_bin.as_deref());
        let updated_recipe = fs::read_to_string(&recipe_path).unwrap();

        if self.assert_recipe_only_expectation(&expectation, &original_recipe, &updated_recipe) {
            return;
        }

        sync_project_to_cook_dir(project.path(), cook_dir.path());
        let output = run_cargo_build(
            cook_dir.path(),
            options.build_bin.as_deref(),
            options.build_package.as_deref(),
            options.build_profile.as_deref(),
            options.target.as_deref(),
            options.target_dir.as_deref(),
        );
        self.assert_build_expectation(expectation, output);
    }

    fn prepare_lockfile_for_first_prepare(
        &self,
        project_root: &std::path::Path,
        modification: &Modification<'_>,
    ) {
        // Keep behavior aligned with real builds: always start from a present,
        // freshly generated lockfile before preparing.
        run_generate_lockfile(project_root);

        if let Modification::UpdateExternalDepInLockfile {
            member,
            dep,
            initial_version,
            ..
        } = modification
        {
            let member_external_deps = self.external_deps(member);
            assert!(
                member_external_deps
                    .iter()
                    .any(|external| external.name == *dep),
                "member {} does not declare external dependency {}",
                member,
                dep
            );
            // Pin the initial lockfile version before applying the tested update.
            run_cargo_update_precise(project_root, dep, initial_version);
        }
    }

    fn run_initial_prepare_cook(
        &self,
        project_root: &std::path::Path,
        recipe_path: &std::path::Path,
        cook_dir: &TempDir,
        options: &RunOptions,
    ) -> String {
        run_prepare(project_root, recipe_path, options.prepare_bin.as_deref());
        let _ = run_cook(
            cook_dir.path(),
            recipe_path,
            options.cook_package.as_deref(),
            options.cook_profile.as_deref(),
            options.target.as_deref(),
            options.target_dir.as_deref(),
        );
        fs::read_to_string(recipe_path).unwrap()
    }

    fn apply_modification(
        &self,
        project_root: &std::path::Path,
        modification: &Modification<'_>,
    ) -> bool {
        match modification {
            Modification::ModifySource { member } => {
                self.modify_source(project_root, member);
                true
            }
            Modification::ModifyBuildScript { member } => {
                self.modify_build_script(project_root, member);
                true
            }
            Modification::AddExternalDep {
                member,
                dep,
                section,
            } => {
                add_external_dep_in_section(project_root, member, dep.name, dep.version, section);
                true
            }
            Modification::AddWorkspaceExternalDep { dep } => {
                add_workspace_external_dep(project_root, dep.name, dep.version);
                true
            }
            Modification::AddExternalDepFeature {
                member,
                dep,
                feature,
            } => {
                add_external_dep_feature(project_root, member, dep, feature);
                true
            }
            Modification::BumpMemberVersion {
                member,
                new_version,
            } => {
                bump_member_version(project_root, member, new_version);
                true
            }
            Modification::BumpLocalDepVersion {
                member,
                dep,
                package,
                new_version,
            } => {
                bump_local_dep_version(project_root, member, dep, *package, new_version);
                true
            }
            Modification::BumpWorkspaceLocalDepVersion { dep, new_version } => {
                bump_workspace_local_dep_version(project_root, dep, new_version);
                true
            }
            Modification::UpdateExternalDepInLockfile {
                dep, new_version, ..
            } => {
                let original_lockfile = fs::read_to_string(project_root.join("Cargo.lock"))
                    .expect("expected Cargo.lock to exist before lockfile update");
                run_cargo_update_precise(project_root, dep, new_version);
                let updated_lockfile = fs::read_to_string(project_root.join("Cargo.lock"))
                    .expect("expected Cargo.lock to exist after lockfile update");
                assert_ne!(
                    original_lockfile, updated_lockfile,
                    "expected Cargo.lock to change after cargo update"
                );
                false
            }
        }
    }

    fn assert_recipe_only_expectation(
        &self,
        expectation: &Expectation,
        original_recipe: &str,
        updated_recipe: &str,
    ) -> bool {
        match expectation {
            Expectation::RecipeChanged => {
                assert_ne!(
                    original_recipe, updated_recipe,
                    "expected recipe to be changed after modification"
                );
                true
            }
            Expectation::RecipeUnchanged => {
                assert_eq!(
                    original_recipe, updated_recipe,
                    "expected recipe to be unchanged after modification"
                );
                true
            }
            _ => false,
        }
    }

    fn assert_build_expectation(&self, expectation: Expectation, output: std::process::Output) {
        let (compiled, _fresh) = parse_compilation_output(&output);
        let workspace_members = workspace_member_names(self);
        let rebuilt_externals = compiled
            .iter()
            .filter(|name| !workspace_members.contains(*name))
            .cloned()
            .collect::<Vec<_>>();
        match expectation {
            Expectation::RecipeChanged | Expectation::RecipeUnchanged => unreachable!(),
            Expectation::BuildSucceeds => {}
            Expectation::ExternalDepsFresh => {
                assert!(
                    rebuilt_externals.is_empty(),
                    "expected no external deps to be rebuilt, but rebuilt: {:?}\n{}",
                    rebuilt_externals,
                    cook_output_string(&output)
                );
            }
            Expectation::ExternalDepsRebuilt => {
                assert!(
                    !rebuilt_externals.is_empty(),
                    "expected at least one external dep to be rebuilt during cargo build\n{}",
                    cook_output_string(&output)
                );
            }
        }
    }
}

fn workspace_member_names(scenario: &Scenario) -> Vec<String> {
    scenario
        .members
        .iter()
        .map(|member| member.name.to_string())
        .collect()
}
