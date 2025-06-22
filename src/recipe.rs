use crate::Skeleton;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    pub skeleton: Skeleton,
}

pub struct TargetArgs {
    pub benches: bool,
    pub tests: bool,
    pub examples: bool,
    pub all_targets: bool,
}

pub enum CommandArg {
    Build,
    Check,
    Clippy,
    Zigbuild,
    NoBuild,
}

pub struct CookArgs {
    pub profile: OptimisationProfile,
    pub command: CommandArg,
    pub default_features: DefaultFeatures,
    pub all_features: AllFeatures,
    pub features: Option<HashSet<String>>,
    pub unstable_features: Option<HashSet<String>>,
    pub target: Option<Vec<String>>,
    pub target_dir: Option<PathBuf>,
    pub target_args: TargetArgs,
    pub manifest_path: Option<PathBuf>,
    pub ignore_manifest: Option<Vec<PathBuf>>,
    pub package: Option<Vec<String>>,
    pub workspace: bool,
    pub offline: bool,
    pub locked: bool,
    pub frozen: bool,
    pub verbose: bool,
    pub timings: bool,
    pub no_std: bool,
    pub bin: Option<Vec<String>>,
    pub bins: bool,
    pub no_build: bool,
}

impl Recipe {
    pub fn prepare(base_path: PathBuf, member: Option<String>) -> Result<Self, anyhow::Error> {
        let skeleton = Skeleton::derive(base_path, member)?;
        Ok(Recipe { skeleton })
    }

    pub fn cook(&self, args: CookArgs) -> Result<(), anyhow::Error> {
        let current_directory = std::env::current_dir()?;
        let ignored_manifests = args
            .ignore_manifest
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|p| current_directory.join(p))
            .collect::<Vec<_>>();

        self.skeleton
            .build_minimum_project(&current_directory, args.no_std, &ignored_manifests)?;
        if args.no_build {
            return Ok(());
        }
        build_dependencies(&args);
        self.skeleton
            .remove_compiled_dummies(
                current_directory,
                args.profile,
                args.target,
                args.target_dir,
                &ignored_manifests,
            )
            .context("Failed to clean up dummy compilation artifacts.")?;
        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum OptimisationProfile {
    Release,
    Debug,
    Other(String),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DefaultFeatures {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AllFeatures {
    Enabled,
    Disabled,
}

fn build_dependencies(args: &CookArgs) {
    let CookArgs {
        profile,
        command: command_arg,
        default_features,
        all_features,
        features,
        unstable_features,
        target,
        target_dir,
        target_args,
        manifest_path,
        ignore_manifest: _ignore_manifest,
        package,
        workspace,
        offline,
        frozen,
        locked,
        verbose,
        timings,
        bin,
        no_std: _no_std,
        bins,
        no_build: _no_build,
    } = args;
    let cargo_path = std::env::var("CARGO").expect("The `CARGO` environment variable was not set. This is unexpected: it should always be provided by `cargo` when invoking a custom sub-command, allowing `cargo-chef` to correctly detect which toolchain should be used. Please file a bug.");
    let mut command = Command::new(cargo_path);
    let command_with_args = match command_arg {
        CommandArg::Build => command.arg("build"),
        CommandArg::Check => command.arg("check"),
        CommandArg::Clippy => command.arg("clippy"),
        CommandArg::Zigbuild => command.arg("zigbuild"),
        CommandArg::NoBuild => return,
    };
    if profile == &OptimisationProfile::Release {
        command_with_args.arg("--release");
    } else if let OptimisationProfile::Other(custom_profile) = profile {
        command_with_args.arg("--profile").arg(custom_profile);
    }
    if default_features == &DefaultFeatures::Disabled {
        command_with_args.arg("--no-default-features");
    }
    if let Some(features) = features {
        let feature_flag = features.iter().cloned().collect::<Vec<String>>().join(",");
        command_with_args.arg("--features").arg(feature_flag);
    }
    if all_features == &AllFeatures::Enabled {
        command_with_args.arg("--all-features");
    }
    if let Some(unstable_features) = unstable_features {
        for unstable_feature in unstable_features.iter().cloned() {
            command_with_args.arg("-Z").arg(unstable_feature);
        }
    }
    if let Some(target) = target {
        for target in target {
            command_with_args.arg("--target").arg(target);
        }
    }
    if let Some(target_dir) = target_dir {
        command_with_args.arg("--target-dir").arg(target_dir);
    }
    if target_args.benches {
        command_with_args.arg("--benches");
    }
    if target_args.tests {
        command_with_args.arg("--tests");
    }
    if target_args.examples {
        command_with_args.arg("--examples");
    }
    if target_args.all_targets {
        command_with_args.arg("--all-targets");
    }
    if let Some(manifest_path) = manifest_path {
        command_with_args.arg("--manifest-path").arg(manifest_path);
    }
    if let Some(package) = package {
        for package in package {
            command_with_args.arg("--package").arg(package);
        }
    }
    if let Some(binary_target) = bin {
        for binary_target in binary_target {
            command_with_args.arg("--bin").arg(binary_target);
        }
    }
    if *workspace {
        command_with_args.arg("--workspace");
    }
    if *offline {
        command_with_args.arg("--offline");
    }
    if *frozen {
        command_with_args.arg("--frozen");
    }
    if *locked {
        command_with_args.arg("--locked");
    }
    if *verbose {
        command_with_args.arg("--verbose");
    }
    if *timings {
        command_with_args.arg("--timings");
    }
    if *bins {
        command_with_args.arg("--bins");
    }

    execute_command(command_with_args);
}

fn execute_command(command: &mut Command) {
    let mut child = command
        .envs(std::env::vars())
        .spawn()
        .expect("Failed to execute process");

    let exit_status = child.wait().expect("Failed to run command");

    if !exit_status.success() {
        match exit_status.code() {
            Some(code) => panic!("Exited with status code: {}", code),
            None => panic!("Process terminated by signal"),
        }
    }
}
