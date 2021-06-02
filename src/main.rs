use anyhow::Context;
use chef::{CookArgs, OptimisationProfile, Recipe};
use clap::crate_version;
use clap::{AppSettings, Clap};
use fs_err as fs;
use std::path::PathBuf;

/// Cache the dependencies of your Rust project.
#[derive(Clap)]
#[clap(
    bin_name = "cargo",
    version = crate_version!(),
    author = "Luca Palmieri <rust@lpalmieri.com>"
)]
pub struct Cli {
    #[clap(subcommand)]
    command: CargoInvocation,
}

#[derive(Clap)]
pub enum CargoInvocation {
    // All `cargo` subcommands receive their name (e.g. `chef` as the first command).
    // See https://github.com/rust-lang/rustfmt/pull/3569
    Chef {
        #[clap(subcommand)]
        command: Command,
    },
}

#[derive(Clap)]
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

#[derive(Clap)]
pub struct Prepare {
    /// The filepath used to save the computed recipe.
    ///
    /// It defaults to "recipe.json".
    #[clap(long, default_value = "recipe.json")]
    recipe_path: PathBuf,
}

#[derive(Clap)]
#[clap(setting = AppSettings::AllowLeadingHyphen, setting = AppSettings::TrailingVarArg)]
pub struct Cook {
    /// The filepath `cook` should be reading the recipe from.
    ///
    /// It defaults to "recipe.json".
    #[clap(long, default_value = "recipe.json")]
    recipe_path: PathBuf,
    /// Build in release mode.
    #[clap(long)]
    release: bool,
    /// Build for the target triple.
    #[clap(long)]
    target: Option<String>,
    /// Directory for all generated artifacts.
    #[clap(long, env = "CARGO_TARGET_DIR")]
    target_dir: Option<PathBuf>,

    /// Options to pass through to `cargo build`.
    #[clap(multiple = true)]
    cargo_options: Vec<String>,
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
            release,
            target,
            target_dir,
            cargo_options,
        }) => {
            if atty::is(atty::Stream::Stdout) {
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

            let profile = if release {
                OptimisationProfile::Release
            } else {
                OptimisationProfile::Debug
            };
            let serialized = fs::read_to_string(recipe_path)
                .context("Failed to read recipe from the specified path.")?;
            let recipe: Recipe =
                serde_json::from_str(&serialized).context("Failed to deserialize recipe.")?;

            recipe
                .cook(CookArgs {
                    profile,
                    target,
                    target_dir,
                    cargo_options,
                })
                .context("Failed to cook recipe.")?;
        }
        Command::Prepare(Prepare { recipe_path }) => {
            let recipe = Recipe::prepare(current_directory).context("Failed to compute recipe")?;
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

#[test]
fn test_pass_through() {
    let args = vec![
        "cargo",
        "chef",
        "cook",
        "--recipe-path",
        "./recipe.json",
        "--no-default-features",
        "--release",
        "testing",
        "--other-options",
        "yes",
    ];
    let cli = Cli::parse_from(args);
    if let Cli {
        command:
            CargoInvocation::Chef {
                command: Command::Cook(Cook { cargo_options, .. }),
            },
    } = cli
    {
        assert_eq!(
            cargo_options,
            // NB: options that chef should know about are only in `cargo_options`
            vec![
                "--no-default-features".into(),
                "--release".into(),
                "testing".to_string(),
                "--other-options".into(),
                "yes".into()
            ]
        )
    }
}
