This directory contains the Docker assets for the pre-built `lukemathwalker/cargo-chef` images.

`docker/Dockerfile` is used by `.github/workflows/docker.yml` to publish images that can be
used as a base layer for `planner` and `builder` stages in downstream Dockerfiles.

## Publishing Workflow (CI)

The workflow has three stages:

1. Resolve inputs (`resolve_inputs`)
- Reads latest git tag and validates it is a release semver (`X.Y.Z`).
- Fetches Rust metadata from Docker Official Images (`library/rust`).
- Keeps all Rust aliases published in `library/rust`.
- Groups aliases by source group key (`GitCommit + ":" + Directory` from `library/rust`).
- Produces one matrix entry per group, each containing:
  - a stable `group_key_tag`
  - a representative Rust tag for building
  - all aliases in that group

2. Build once per group (`build_unique_images`)
- Builds exactly one canonical image per `(cargo-chef version, group key)`:
  - `<cargo-chef version>-base-<group_key_tag>`
- Uses one representative Rust alias as build input (prefers versioned tags when present).
- Uses upstream `Architectures` to set `buildx` platforms, limited to `amd64`, `arm64`, `arm/v7`, and `386`.
- Skips if the canonical image already exists.

3. Publish aliases per group (`publish_group_aliases`)
- For each group, publishes all aliases in one `imagetools create` call:
  - `<cargo-chef version>-rust-<alias>` for each alias
  - `latest-rust-<alias>` for each alias
  - plus global `latest` when `latest` belongs to the group
- Uses the canonical image as source and does not rebuild layers.
- Re-applies alias tags on each run instead of pre-checking each alias.

## Design Constraints

The workflow is designed to satisfy all of the following:

- Publish new images for new cargo-chef releases.
- Keep short and full Rust aliases (`1`, `1.93`, `1.93.1`) available.
- Avoid redundant builds when multiple aliases point to the same upstream Rust source group.
- Avoid per-alias race conditions by publishing aliases in grouped batches.
- Avoid per-alias upstream manifest inspection in Docker Hub.
- Minimize the number of read operations via Docker API calls, to avoid rate limits.
