<h1 align="center">cargo-chef</h1>
<div align="center">
 <strong>
   Cache the dependencies of your Rust project and speed up your Docker builds.
 </strong>
</div>

<br />

<div align="center">
  <!-- Crates version -->
  <a href="https://crates.io/crates/cargo-chef">
    <img src="https://img.shields.io/crates/v/cargo-chef.svg?style=flat-square"
    alt="Crates.io version" />
  </a>
  <!-- Downloads -->
  <a href="https://crates.io/crates/cargo-chef">
    <img src="https://img.shields.io/crates/d/cargo-chef.svg?style=flat-square"
      alt="Download" />
  </a>
</div>
<br/>

# Table of Contents
0. [How to install](#how-to-install)
1. [How to use](#how-to-use)
2. [Limitations](#limitations)
3. [License](#license)

## How To Install 

You can install `cargo-chef` from [crates.io](https://crates.io) with

```bash
cargo install cargo-chef
```

## How to use

`cargo-chef` exposes two commands: `prepare` and `cook`:

```bash
cargo chef --help
```

```text

cargo-chef

USAGE:
    cargo chef <SUBCOMMAND>

SUBCOMMANDS:
    cook       Re-hydrate the minimum project skeleton identified by `cargo chef prepare` and
               build it to cache dependencies
    prepare    Analyze the current project to determine the minimum subset of files (Cargo.lock
               and Cargo.toml manifests) required to build it and cache dependencies
```

`prepare` examines your project and builds a _recipe_ that captures the set of information required to build your dependencies.

```bash
cargo chef prepare --recipe-path recipe.json
```

Nothing too mysterious going on here, you can examine the `recipe.json` file: it contains the skeleton of your project (e.g. all the `Cargo.toml` files with their relative path, the `Cargo.lock` file is available) plus a few additional pieces of information.  
 In particular it makes sure that all libraries and binaries are explicitly declared in their respective `Cargo.toml` files even if they can be found at the canonical default location (`src/main.rs` for a binary, `src/lib.rs` for a library).
 
The `recipe.json` is the equivalent of the Python `requirements.txt` file - it is the only input required for `cargo chef cook`, the command that will build out our dependencies:

```bash
cargo chef cook --recipe-path recipe.json
```

If you want to build in `--release` mode:

```bash
cargo chef cook --release --recipe-path recipe.json
```

You can leverage it in a Dockerfile:

```dockerfile
FROM rust as cacher
WORKDIR app
# We only pay the installation cost once, 
# it will be cached from the second build onwards
RUN cargo install cargo-chef
# Build the recipe.json beforehand/check it in version control
COPY recipe.json .
RUN cargo chef cook --release --recipe-path recipe.json

FROM rust as builder
WORKDIR app
# Copy over the source code
COPY . .
# Copy over the cached dependencies
COPY --from=cacher /app/target target
RUN cargo build --release --bin app

FROM rust as runtime
WORKDIR app
COPY --from=builder /app/target/release/app /usr/local/bin
ENTRYPOINT ["./usr/local/bin/app"]
```

We are using three stages: the first caches our dependencies, the second builds the binary and the third is our runtime environment.  
As long as your dependencies do not change, all steps up to `cargo build --release --bin app` will be cached by Docker, massively speeding up your builds (up to 5x measured on some commercial projects).

## Limitations

`cargo-chef` has been tested on a few OpenSource projects and some of commercial projects, but our testing has definitely not exhausted the range of possibilities when it comes to `cargo build` customisations and we are sure that there are a few rough edges that will have to be smoothed out - please file issues on [GitHub](https://github.com/LukeMathWalker/cargo-chef).

So far we have found the following limitations and caveats:

- `cargo cook` and `cargo build` must be executed from the same working directory. If you examine the `*.d` files under `target/debug/deps` for one of your projects using `cat` you will notice that they contain absolute paths referring to the project `target` directory. If moved around, `cargo` will not leverage them as cached dependencies;
- `cargo build` will build local dependencies (outside of the current project) from scratch, even if they are unchanged, due to the reliance of its fingerprinting logic on timestamps (see [this _long_ issue on `cargo`'s repository](https://github.com/rust-lang/cargo/issues/2));
- `cargo build` will build dependencies fetched from private registries (i.e. not crates.io) from scratch. Still investigating why, probably related to the above-mentioned fingerprinting algorithm.

`cargo-chef` has not yet been tested extensively with projects leveraging build files.

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
