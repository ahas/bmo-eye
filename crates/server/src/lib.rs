//! The vision unit's server: its routes, its store, the chain over the local engine nodes and the hosted
//! backends, the media a look is made of, and the tool's answer to a call of the tool host's
//! (`docs/api.md`, `docs/modes.md`, `docs/plan.md`).

pub mod chain;
pub mod config;
pub mod media;
pub mod openapi;
pub mod routes;
pub mod store;
pub mod tool;

/// The version the service's health answers: the set's version the build was made at, which the testbed
/// hands the build as `BMO_VERSION`, else the manifest's placeholder (bmo-chat's `docs/conventions.md`,
/// 「Versions」).
pub const VERSION: &str = match option_env!("BMO_VERSION") {
  Some(version) => version,
  None => env!("CARGO_PKG_VERSION"),
};
