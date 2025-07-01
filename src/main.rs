use anyhow::{anyhow, Context};
use chef::{
    AllFeatures, CommandArg, CookArgs, DefaultFeatures, OptimisationProfile, Recipe, TargetArgs,
};
use clap::crate_version;
use clap::Parser;
use fs_err as fs;
use std::collections::HashSet;
use std::io::IsTerminal;
use std::path::PathBuf;

/// Cache the dependencies of your Rust project.
#[derive(Parser)]
#[command(
    bin_name = "cargo",
    version = crate_version!(),
    author = "Luca Palmieri <rust@lpalmieri.com>"
)]
pub struct Cli {
    #[command(subcommand)]
    command: CargoInvocation,
}

#[derive(Parser)]
pub enum CargoInvocation {
    // All `cargo` subcommands receive their name (e.g. `chef` as the first command).
    // See https://github.com/rust-lang/rustfmt/pull/3569
    Chef {
        #[command(subcommand)]
        command: Command,
    },
}

#[derive(Parser)]
#[command(
    version = crate_version!(),
    author = "Luca Palmieri <rust@lpalmieri.com>"
)]
pub enum Command {
    /// Analyze the current project to determine the minimum subset of files (Cargo.lock and
    /// Cargo.toml manifests) required to build it and cache dependencies.
    ///
    /// `cargo chef prepare` emits a recipe file that can be later used via
    /// `cargo chef cook --recipe <recipe-path>.json`.
    Prepare(Prepare),
    /// Re-hydrate the minimum project skeleton identified by `cargo chef prepare` and build
    /// it to cache dependencies.
    Cook(Cook),
}

#[derive(Parser)]
pub struct Prepare {
    /// The filepath used to save the computed recipe.
    ///
    /// It defaults to "recipe.json".
    #[arg(long, default_value = "recipe.json")]
    recipe_path: PathBuf,

    /// When --bin is specified, `cargo-chef` will ignore all members of the workspace
    /// that are not necessary to successfully compile the specific binary.
    #[arg(long)]
    bin: Option<String>,
}

#[derive(Parser)]
pub struct Cook {
    /// The filepath `cook` should be reading the recipe from.
    ///
    /// It defaults to "recipe.json".
    #[arg(long, default_value = "recipe.json")]
    recipe_path: PathBuf,
    /// Build artifacts with the specified profile.
    #[arg(long)]
    profile: Option<String>,
    /// Build in release mode.
    #[arg(long)]
    release: bool,
    /// Run `cargo check` instead of `cargo build`. Primarily useful for speeding up your CI pipeline.
    #[arg(long)]
    check: bool,
    /// Run `cargo clippy` instead of `cargo build`. Primarily useful for speeding up your CI pipeline. Requires clippy to be installed.
    #[arg(long)]
    clippy: bool,
    /// Build for the target triple. The flag can be passed multiple times to cook for multiple targets.
    #[arg(long)]
    target: Option<Vec<String>>,
    /// Directory for all generated artifacts.
    #[arg(long, env = "CARGO_TARGET_DIR")]
    target_dir: Option<PathBuf>,
    /// Do not activate the `default` feature.
    #[arg(long)]
    no_default_features: bool,
    /// Enable all features.
    #[arg(long)]
    all_features: bool,
    /// Space or comma separated list of features to activate.
    #[arg(long, value_delimiter = ',')]
    features: Option<Vec<String>>,
    /// Unstable feature to activate (only available on the nightly channel).
    #[arg(short = 'Z')]
    unstable_features: Option<Vec<String>>,
    /// Build all benches
    #[arg(long)]
    benches: bool,
    /// Build all tests
    #[arg(long)]
    tests: bool,
    /// Build all examples
    #[arg(long)]
    examples: bool,
    /// Build all targets.
    /// This is equivalent to specifying `--tests --benches --examples`.
    #[arg(long)]
    all_targets: bool,
    /// Path to Cargo.toml
    #[arg(long)]
    manifest_path: Option<PathBuf>,
    /// Package(s) to build (see `cargo help pkgid`)
    #[arg(long, short = 'p')]
    package: Option<Vec<String>>,
    /// Build all members in the workspace.
    #[arg(long)]
    workspace: bool,
    /// Build offline.
    #[arg(long)]
    offline: bool,
    /// Require Cargo.lock is up to date
    #[arg(long)]
    locked: bool,
    /// Use verbose output
    #[arg(long, short = 'v')]
    verbose: bool,
    /// Require Cargo.lock and cache are up to date
    #[arg(long)]
    frozen: bool,
    /// Report build timings.
    #[arg(long)]
    timings: bool,
    /// Cook using `#[no_std]` configuration  (does not affect `proc-macro` crates)
    #[arg(long)]
    no_std: bool,
    /// Build only the specified binary. This can be specified with multiple binaries.
    #[arg(long)]
    bin: Option<Vec<String>>,
    /// Build all binaries and ignore everything else.
    #[arg(long)]
    bins: bool,
    /// Run `cargo zigbuild` instead of `cargo build`. You need to install
    /// the `cargo-zigbuild` crate and the Zig compiler toolchain separately
    #[arg(long)]
    zigbuild: bool,
    /// Modify the current workspace to maximise cache reuse, but don't invoke `cargo build`.
    /// This option exist to leverage `cargo-chef` when trying to cache dependencies in Rust
    /// projects that rely on a custom build system (i.e. not `cargo`).
    #[clap(long)]
    no_build: bool,
    /// Number of rust workers
    #[clap(long)]
    jobs: Option<u16>,
}

fn _main() -> Result<(), anyhow::Error> {
    let current_directory = std::env::current_dir().unwrap();

    let cli = Cli::parse();
    // "Unwrapping" the actual command.
    let command = match cli.command {
        CargoInvocation::Chef { command } => command,
    };

    match command {
        Command::Cook(Cook {
            recipe_path,
            profile,
            release,
            check,
            clippy,
            target,
            no_default_features,
            all_features,
            features,
            unstable_features,
            target_dir,
            benches,
            tests,
            examples,
            all_targets,
            manifest_path,
            package,
            workspace,
            offline,
            frozen,
            locked,
            verbose,
            timings,
            no_std,
            bin,
            zigbuild,
            bins,
            no_build,
            jobs,
        }) => {
            if std::io::stdout().is_terminal() {
                eprintln!("WARNING stdout appears to be a terminal.");
                eprintln!(
                    "cargo-chef is not meant to be run in an interactive environment \
                and will overwrite some existing files (namely any `lib.rs`, `main.rs` and \
                `Cargo.toml` it finds)."
                );
                eprintln!();
                eprint!("To continue anyway, type `yes`: ");

                let mut answer = String::with_capacity(3);
                std::io::stdin()
                    .read_line(&mut answer)
                    .context("Failed to read from stdin")?;

                if "yes" != answer.trim() {
                    std::process::exit(1);
                }
            }

            let features: Option<HashSet<String>> = features.and_then(|features| {
                if features.is_empty() {
                    None
                } else {
                    Some(features.into_iter().collect())
                }
            });

            let unstable_features: Option<HashSet<String>> =
                unstable_features.and_then(|unstable_features| {
                    if unstable_features.is_empty() {
                        None
                    } else {
                        Some(unstable_features.into_iter().collect())
                    }
                });

            let profile = match (release, profile) {
                (false, None) =>  OptimisationProfile::Debug,
                (false, Some(profile)) if profile == "dev" => OptimisationProfile::Debug,
                (true, None) => OptimisationProfile::Release,
                (false, Some(profile)) if profile == "release" => OptimisationProfile::Release,
                (false, Some(custom_profile)) => OptimisationProfile::Other(custom_profile),
                (true, Some(_)) => Err(anyhow!("You specified both --release and --profile arguments. Please remove one of them, or both"))?
            };
            let command = match (check, clippy, zigbuild, no_build) {
                (true, false, false, false) => CommandArg::Check,
                (false, true, false, false) => CommandArg::Clippy,
                (false, false, true, false) => CommandArg::Zigbuild,
                (false, false, false, true) => CommandArg::NoBuild,
                (false, false, false, false) => CommandArg::Build,
                _ => Err(anyhow!("Only one (or none) of the  `clippy`, `check`, `zigbuild`, and `no-build` arguments are allowed. Please remove some of them, or all"))?,
            };

            let default_features = if no_default_features {
                DefaultFeatures::Disabled
            } else {
                DefaultFeatures::Enabled
            };

            let all_features = if all_features {
                AllFeatures::Enabled
            } else {
                AllFeatures::Disabled
            };

            let serialized = fs::read_to_string(recipe_path)
                .context("Failed to read recipe from the specified path.")?;
            let recipe: Recipe =
                serde_json::from_str(&serialized).context("Failed to deserialize recipe.")?;
            let target_args = TargetArgs {
                benches,
                tests,
                examples,
                all_targets,
            };
            recipe
                .cook(CookArgs {
                    profile,
                    command,
                    default_features,
                    all_features,
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
                    no_std,
                    bin,
                    locked,
                    frozen,
                    verbose,
                    bins,
                    no_build,
                    jobs,
                })
                .context("Failed to cook recipe.")?;
        }
        Command::Prepare(Prepare { recipe_path, bin }) => {
            let recipe =
                Recipe::prepare(current_directory, bin).context("Failed to compute recipe")?;
            let serialized =
                serde_json::to_string(&recipe).context("Failed to serialize recipe.")?;
            fs::write(recipe_path, serialized).context("Failed to save recipe to 'recipe.json'")?;
        }
    }
    Ok(())
}

fn main() -> Result<(), anyhow::Error> {
    env_logger::init();
    _main()
}
