This directory contains the Docker assets for the pre-built `lukemathwalker/cargo-chef` images.

`docker/Dockerfile` is used by `.github/workflows/docker.yml` to publish images that can be
used as a base layer for `planner` and `builder` stages in downstream Dockerfiles.

## Publishing Workflow (CI)

The image publishing workflow lives at `.github/workflows/docker.yml`.

The workflow has three stages:

1. Resolve inputs (`resolve_inputs`)
- Reads latest git tag and validates it is a release semver (`X.Y.Z`).
- Fetches Rust tags from Docker Official Images metadata.
- Keeps only tags we support:
  - `latest`
  - short aliases (`1`, `1.84`, ...)
  - full versions (`1.84.0`, ...)
  - optional distro suffixes (`-slim`, `-alpine`)
- Resolves each Rust tag to its current manifest digest.
- Produces two matrices:
  - `unique_digest_matrix` (one entry per unique digest)
  - `tag_matrix` (one entry per published Rust tag alias)

2. Build once per unique digest (`build_unique_images`)
- Builds exactly one image for each unique `(cargo-chef version, Rust digest)` pair.
- Uses `BASE_IMAGE=rust@sha256:...` to pin the resolved Rust content.
- Publishes a canonical internal tag:
  - `<cargo-chef version>-base-<digest without sha256:>`
- Skips if the canonical tag already exists.

3. Publish aliases (`publish_aliases`)
- For every Rust tag alias, publishes user-facing tags from the canonical image:
  - `<cargo-chef version>-rust-<rust tag>`
  - `latest-rust-<rust tag>`
  - plus `latest` when `<rust tag> == latest`
- Uses `docker buildx imagetools create`, which publishes tags without rebuilding layers.
- Skips alias publication if all required aliases already exist.

## Design Constraints

The workflow tries to satisfy the following goals:

- Publish new images when a new Rust image release changes the underlying digest.
- Publish new images for new cargo-chef releases.
- Keep short and full Rust aliases (`1`, `1.84`, `1.84.0`) available.
- Avoid redundant builds when multiple aliases point to the same Rust digest.
- Avoid per-alias race conditions by building from a deduplicated digest matrix.
