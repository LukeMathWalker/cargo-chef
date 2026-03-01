use crate::helpers::{
    DependencySection, Expectation, ExternalDepSpec, Member, Modification, RunOptions, Scenario,
};
use rstest::rstest;

#[rstest]
#[case(base_scenario(), Modification::ModifySource { member: "a" }, Expectation::ExternalDepsFresh)]
#[case(
    multi_member_scenario(),
    Modification::ModifySource { member: "a" },
    Expectation::ExternalDepsFresh
)]
#[case(
    base_scenario(),
    Modification::AddExternalDep {
        member: "a",
        dep: ExternalDepSpec {
            name: "ryu",
            version: "1",
        },
        section: DependencySection::Dependencies,
    },
    Expectation::ExternalDepsRebuilt
)]
#[case(
    multi_member_scenario(),
    Modification::AddExternalDep {
        member: "a",
        dep: ExternalDepSpec {
            name: "ryu",
            version: "1",
        },
        section: DependencySection::Dependencies,
    },
    Expectation::ExternalDepsRebuilt
)]
fn external_dependency_rebuild_behaviour(
    #[case] scenario: Scenario,
    #[case] modification: Modification<'_>,
    #[case] expectation: Expectation,
) {
    scenario.run(modification, expectation);
}

#[test]
fn recipe_changes_when_workspace_member_dependencies_change() {
    let scenario = Scenario::workspace()
        .member(Member::lib("a").local_dep("b"))
        .member(Member::lib("b"));
    scenario.run(
        Modification::AddExternalDep {
            member: "a",
            dep: ExternalDepSpec {
                name: "ryu",
                version: "1",
            },
            section: DependencySection::Dependencies,
        },
        Expectation::RecipeChanged,
    );
}

#[test]
fn recipe_is_unchanged_when_workspace_member_version_changes() {
    let scenario = Scenario::workspace()
        .member(Member::lib("a").local_dep("b"))
        .member(Member::lib("b").version("0.2.3"));
    scenario.run(
        Modification::BumpMemberVersion {
            member: "b",
            new_version: "9.9.9",
        },
        Expectation::RecipeUnchanged,
    );
}

#[test]
fn recipe_changes_when_lockfile_updates_external_dependency() {
    let scenario = Scenario::workspace().member(Member::lib("a").external_dep("itoa@1"));
    scenario.run(
        Modification::UpdateExternalDepInLockfile {
            member: "a",
            dep: "itoa",
            initial_version: "1.0.10",
            new_version: "1.0.17",
        },
        Expectation::RecipeChanged,
    );
}

#[test]
fn recipe_is_unchanged_when_local_dependency_version_changes() {
    let scenario = Scenario::workspace()
        .member(Member::lib("a").local_dep("b@0.1.0"))
        .member(Member::lib("b"));
    scenario.run(
        Modification::BumpLocalDepVersion {
            member: "a",
            dep: "b",
            package: None,
            new_version: "9.9.9",
        },
        Expectation::RecipeUnchanged,
    );
}

#[test]
fn recipe_is_unchanged_for_unrelated_members_when_prepare_targets_single_bin() {
    let scenario = Scenario::workspace()
        .member(
            Member::lib("app")
                .external_dep("itoa@1")
                .local_dep("shared")
                .with_bin("app_cli"),
        )
        .member(Member::lib("shared").external_dep("itoa@1"))
        .member(
            Member::bin("ignored_cli")
                .local_dep("ignored_local")
                .version("0.1.0"),
        )
        .member(
            Member::lib("ignored_local")
                .version("0.2.0")
                .external_dep("ryu@1"),
        );

    scenario.run_with_options(
        Modification::AddExternalDep {
            member: "ignored_local",
            dep: ExternalDepSpec {
                name: "ryu",
                version: "1",
            },
            section: DependencySection::Dependencies,
        },
        Expectation::RecipeUnchanged,
        RunOptions::for_bin("app_cli"),
    );
}

#[test]
fn external_deps_are_not_rebuilt_for_selected_package_when_unrelated_member_changes() {
    let scenario = Scenario::workspace()
        .member(
            Member::lib("app")
                .local_dep("shared")
                .external_dep("itoa@1"),
        )
        .member(Member::lib("shared").external_dep("itoa@1"))
        .member(Member::lib("unrelated").external_dep("ryu@1"));

    scenario.run_with_options(
        Modification::ModifySource {
            member: "unrelated",
        },
        Expectation::ExternalDepsFresh,
        RunOptions::for_package("app"),
    );
}

#[test]
fn recipe_changes_when_build_script_member_dependencies_change() {
    let scenario = Scenario::workspace().member(
        Member::lib("app")
            .external_dep("itoa@1")
            .with_build_script(),
    );

    scenario.run(
        Modification::AddExternalDep {
            member: "app",
            dep: ExternalDepSpec {
                name: "ryu",
                version: "1",
            },
            section: DependencySection::Dependencies,
        },
        Expectation::RecipeChanged,
    );
}

#[test]
fn recipe_changes_when_dependency_features_change() {
    let scenario = Scenario::workspace().member(Member::lib("app").external_dep("serde@1"));

    scenario.run(
        Modification::AddExternalDepFeature {
            member: "app",
            dep: "serde",
            feature: "derive",
        },
        Expectation::RecipeChanged,
    );
}

#[test]
fn external_deps_are_not_rebuilt_when_build_script_source_changes() {
    let scenario = Scenario::workspace().member(
        Member::lib("app")
            .external_dep("itoa@1")
            .with_build_script(),
    );

    scenario.run(
        Modification::ModifyBuildScript { member: "app" },
        Expectation::ExternalDepsFresh,
    );
}

#[test]
fn recipe_changes_when_build_dependencies_change_for_build_script_member() {
    let scenario = Scenario::workspace().member(
        Member::lib("app")
            .external_dep("itoa@1")
            .with_build_script(),
    );

    scenario.run(
        Modification::AddExternalDep {
            member: "app",
            dep: ExternalDepSpec {
                name: "ryu",
                version: "1",
            },
            section: DependencySection::BuildDependencies,
        },
        Expectation::RecipeChanged,
    );
}

#[test]
fn recipe_changes_when_transitive_local_dep_changes_for_selected_bin() {
    let scenario = Scenario::workspace()
        .member(
            Member::lib("app")
                .local_dep("mid")
                .external_dep("itoa@1")
                .with_bin("app_cli"),
        )
        .member(Member::lib("mid").local_dep("leaf"))
        .member(Member::lib("leaf").external_dep("ryu@1"));

    scenario.run_with_options(
        Modification::AddExternalDep {
            member: "leaf",
            dep: ExternalDepSpec {
                name: "itoa",
                version: "1",
            },
            section: DependencySection::Dependencies,
        },
        Expectation::RecipeChanged,
        RunOptions::for_bin("app_cli"),
    );
}

#[rstest]
#[case(Expectation::RecipeUnchanged)]
#[case(Expectation::ExternalDepsFresh)]
fn selected_bin_transitive_source_change_expectations(#[case] expectation: Expectation) {
    selected_bin_transitive_scenario().run_with_options(
        Modification::ModifySource { member: "leaf" },
        expectation,
        RunOptions::for_bin("app_cli"),
    );
}

#[test]
fn recipe_changes_when_workspace_dependency_external_version_changes() {
    let scenario = Scenario::workspace()
        .workspace_external_dep("itoa@1")
        .member(Member::lib("app").workspace_dep("itoa"));
    scenario.run(
        Modification::AddWorkspaceExternalDep {
            dep: ExternalDepSpec {
                name: "ryu",
                version: "1",
            },
        },
        Expectation::RecipeChanged,
    );
}

#[test]
fn recipe_is_unchanged_when_workspace_dependency_local_version_changes() {
    let scenario = Scenario::workspace()
        .workspace_local_dep("shared@0.2.0")
        .member(Member::lib("app").workspace_dep("shared"))
        .member(Member::lib("shared").version("0.2.0"));
    scenario.run(
        Modification::BumpWorkspaceLocalDepVersion {
            dep: "shared",
            new_version: "9.9.9",
        },
        Expectation::RecipeUnchanged,
    );
}

#[test]
fn recipe_is_unchanged_when_renamed_local_dependency_version_changes() {
    let scenario = Scenario::workspace()
        .member(Member::lib("a").renamed_local_dep("renamed_b", "b", "0.1.0"))
        .member(Member::lib("b").version("0.1.0"));
    scenario.run(
        Modification::BumpLocalDepVersion {
            member: "a",
            dep: "renamed_b",
            package: Some("b"),
            new_version: "9.9.9",
        },
        Expectation::RecipeUnchanged,
    );
}

#[rstest]
#[case("dev")]
#[case("test")]
#[case("release")]
fn build_succeeds_after_cook_for_profiles(#[case] profile: &str) {
    let scenario = Scenario::workspace().member(Member::lib("app"));
    scenario.run_with_options(
        Modification::ModifySource { member: "app" },
        Expectation::BuildSucceeds,
        RunOptions {
            cook_profile: Some(profile.to_string()),
            build_profile: Some(profile.to_string()),
            ..RunOptions::default()
        },
    );
}

#[test]
fn build_succeeds_after_cook_with_custom_target_dir_and_target() {
    let scenario = Scenario::workspace().member(Member::lib("app"));
    let target = host_target();
    scenario.run_with_options(
        Modification::ModifySource { member: "app" },
        Expectation::BuildSucceeds,
        RunOptions {
            target: Some(target),
            target_dir: Some("target-cache-custom".to_string()),
            ..RunOptions::default()
        },
    );
}

#[rstest]
#[case(DependencySection::Dependencies)]
#[case(DependencySection::DevDependencies)]
#[case(DependencySection::BuildDependencies)]
#[case(DependencySection::TargetDependencies { cfg: "cfg(unix)" })]
fn recipe_changes_when_external_dep_changes_in_manifest_section(
    #[case] section: DependencySection<'_>,
) {
    let scenario = Scenario::workspace().member(Member::lib("app").external_dep("itoa@1"));
    scenario.run(
        Modification::AddExternalDep {
            member: "app",
            dep: ExternalDepSpec {
                name: "ryu",
                version: "1",
            },
            section,
        },
        Expectation::RecipeChanged,
    );
}

fn host_target() -> String {
    let output = std::process::Command::new("rustc")
        .arg("-vV")
        .output()
        .expect("rustc -vV must run");
    let stdout = String::from_utf8(output.stdout).expect("rustc output must be utf-8");
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .expect("host target must be present")
        .to_string()
}

fn base_scenario() -> Scenario {
    Scenario::workspace().member(Member::lib("a").external_dep("itoa@1"))
}

fn multi_member_scenario() -> Scenario {
    Scenario::workspace()
        .member(Member::lib("a").local_dep("b").external_dep("itoa@1"))
        .member(Member::lib("b").external_dep("itoa@1"))
}

fn selected_bin_transitive_scenario() -> Scenario {
    Scenario::workspace()
        .member(
            Member::lib("app")
                .local_dep("mid")
                .external_dep("itoa@1")
                .with_bin("app_cli"),
        )
        .member(Member::lib("mid").local_dep("leaf"))
        .member(Member::lib("leaf").external_dep("ryu@1"))
}

#[test]
fn recipe_unchanged_when_unrelated_member_dep_added_for_selected_bin() {
    // app depends on shared (both use itoa)
    // other uses ryu and is not needed by app
    // Adding a dep to other should not affect app's recipe
    let scenario = Scenario::workspace()
        .member(
            Member::lib("app")
                .local_dep("shared")
                .external_dep("itoa@1")
                .with_bin("app_cli"),
        )
        .member(Member::lib("shared").external_dep("itoa@1"))
        .member(Member::lib("other").external_dep("ryu@1"));

    scenario.run_with_options(
        Modification::AddExternalDep {
            member: "other",
            dep: ExternalDepSpec {
                name: "itoa",
                version: "1",
            },
            section: DependencySection::Dependencies,
        },
        Expectation::RecipeUnchanged,
        RunOptions::for_bin("app_cli"),
    );
}
