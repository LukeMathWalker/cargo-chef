# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.76](https://github.com/LukeMathWalker/cargo-chef/compare/v0.1.75...v0.1.76) - 2026-03-03

### Added

- Minimize generated recipe to increase cache hit ratio when `cargo chef prepare` is invoked with a `--bin` option (by [@preiter93](https://github.com/preiter93))
- Publish a prebuilt `cargo-chef` Docker image for every upstream Rust tag. 
- Broaden the set of supported architectures for Docker images to include `i386` and `arm32v7`

### Other

- Upgrade to latest versions of all dependencies
- Allow cargo-chef to fetch dependencies in `cargo chef prepare`, if either `--bin` was specified or
  the lockfile is missing.

## [0.1.75](https://github.com/LukeMathWalker/cargo-chef/compare/v0.1.74...v0.1.75) - 2026-02-28

### Added

- Support the --jobs flags.

### Other

- Disable semver check. We version based on the CLI interface, not the library one
- Bump rustsec/audit-check from 1.4.1 to 2.0.0 ([#278](https://github.com/LukeMathWalker/cargo-chef/pull/278))
- Use a PAT to allow release-plz's job to trigger other workflows

## [0.1.74](https://github.com/LukeMathWalker/cargo-chef/compare/v0.1.73...v0.1.74) - 2026-02-27

### Other

- Fix Docker publishing workflow ([#328](https://github.com/LukeMathWalker/cargo-chef/pull/328))
- Start testing caching behaviour with real builds ([#327](https://github.com/LukeMathWalker/cargo-chef/pull/327))
