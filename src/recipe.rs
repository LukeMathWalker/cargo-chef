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
}

pub struct CookArgs {
    pub profile: OptimisationProfile,
    pub command: CommandArg,
    pub default_features: DefaultFeatures,
    pub features: Option<HashSet<String>>,
    pub unstable_features: Option<HashSet<String>>,
    pub target: Option<Vec<String>>,
    pub target_dir: Option<PathBuf>,
    pub target_args: TargetArgs,
    pub manifest_path: Option<PathBuf>,
    pub package: Option<String>,
    pub workspace: bool,
    pub offline: bool,
    pub timings: bool,
    pub no_std: bool,
    pub bin: Option<String>,
}

impl Recipe {
    pub fn prepare(base_path: PathBuf, member: Option<String>) -> Result<Self, anyhow::Error> {
        let skeleton = Skeleton::derive(base_path, member)?;
        Ok(Recipe { skeleton })
    }

    pub fn cook(&self, args: CookArgs) -> Result<(), anyhow::Error> {
        let current_directory = std::env::current_dir()?;
        self.skeleton
            .build_minimum_project(&current_directory, args.no_std)?;
        build_dependencies(&args);
        self.skeleton
            .remove_compiled_dummies(
                current_directory,
                args.profile,
                args.target,
                args.target_dir,
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

fn build_dependencies(args: &CookArgs) {
    let CookArgs {
        profile,
        command: command_arg,
        default_features,
        features,
        unstable_features,
        target,
        target_dir,
        target_args,
        manifest_path,
        package,
        workspace,
        offline,
        timings,
        bin,
        no_std: _no_std,
    } = args;
    let cargo_path = std::env::var("CARGO").expect("The `CARGO` environment variable was not set. This is unexpected: it should always be provided by `cargo` when invoking a custom sub-command, allowing `cargo-chef` to correctly detect which toolchain should be used. Please file a bug.");
    let mut command = Command::new(cargo_path);
    let command_with_args = match command_arg {
        CommandArg::Build => command.arg("build"),
        CommandArg::Check => command.arg("check"),
        CommandArg::Clippy => command.arg("clippy"),
        CommandArg::Zigbuild => command.arg("zigbuild"),
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
        command_with_args.arg("--package").arg(package);
    }
    if let Some(binary_target) = bin {
        command_with_args.arg("--bin").arg(binary_target);
    }
    if *workspace {
        command_with_args.arg("--workspace");
    }
    if *offline {
        command_with_args.arg("--offline");
    }
    if *timings {
        command_with_args.arg("--timings");
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
