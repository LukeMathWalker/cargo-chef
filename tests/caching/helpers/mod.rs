//! Test-only tooling for cache behavior scenarios.
//!
//! This module is the public surface for the `tests/caching` test crate. It
//! re-exports types and helpers used by the tests, while keeping the internal
//! implementation split across focused submodules.
//!
//! Use [`model::Scenario::workspace`] to define a [`model::Scenario`] and then call
//! [`model::Scenario::run`] with a [`model::Modification`] and
//! [`model::Expectation`].

mod cargo;
mod fs;
mod manifest;
pub(crate) mod model;
mod project;
mod steps;

pub(crate) use model::DependencySection;
pub(crate) use model::Expectation;
pub(crate) use model::ExternalDepSpec;
pub(crate) use model::Member;
pub(crate) use model::Modification;
pub(crate) use model::RunOptions;
pub(crate) use model::Scenario;
