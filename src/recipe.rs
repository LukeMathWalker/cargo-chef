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

impl Recipe {
    pub fn prepare(base_path: PathBuf) -> Result<Self, anyhow::Error> {
        let skeleton = Skeleton::derive(&base_path)?;
        Ok(Recipe { skeleton })
    }

    pub fn cook(
        &self,
        profile: OptimisationProfile,
        default_features: DefaultFeatures,
        features: Option<HashSet<String>>,
        target: Option<String>,
        target_dir: Option<PathBuf>,
        target_args: TargetArgs,
    ) -> Result<(), anyhow::Error> {
        let current_directory = std::env::current_dir()?;
        self.skeleton.build_minimum_project(&current_directory)?;
        build_dependencies(
            profile,
            default_features,
            features,
            &target,
            &target_dir,
            target_args,
        );
        self.skeleton
            .remove_compiled_dummy_libraries(current_directory, profile, target, target_dir)
            .context("Failed to clean up dummy compilation artifacts.")?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum OptimisationProfile {
    Release,
    Debug,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DefaultFeatures {
    Enabled,
    Disabled,
}

fn build_dependencies(
    profile: OptimisationProfile,
    default_features: DefaultFeatures,
    features: Option<HashSet<String>>,
    target: &Option<String>,
    target_dir: &Option<PathBuf>,
    target_args: TargetArgs,
) {
    let mut command = Command::new("cargo");
    let command_with_args = command.arg("build");
    if profile == OptimisationProfile::Release {
        command_with_args.arg("--release");
    }
    if default_features == DefaultFeatures::Disabled {
        command_with_args.arg("--no-default-features");
    }
    if let Some(features) = features {
        let feature_flag = features.into_iter().collect::<Vec<_>>().join(",");
        command_with_args.arg("--features").arg(feature_flag);
    }
    if let Some(target) = target {
        command_with_args.arg("--target").arg(target);
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
