//! Selected original reRBR file readers. See UPSTREAM.md for hashes and scope.
//! No game data, executable loader, renderer, physics or platform integration.

// The shared upstream reader retains helpers used by formats outside this subset.
#[cfg(test)]
mod damage;
#[allow(dead_code)]
mod le;

pub mod dds;
pub mod ini;
pub mod lbs;
pub mod rbz;
pub mod stage_textures;
