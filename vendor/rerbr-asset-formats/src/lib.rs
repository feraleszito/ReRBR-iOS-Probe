//! Selected original reRBR file readers. See UPSTREAM.md for hashes and scope.
//! No game data, executable loader, renderer, physics or platform integration.

// The shared upstream reader retains helpers used by formats outside this subset.
#[cfg(test)]
mod damage;
#[allow(dead_code)]
mod le;

pub mod car_parts;
pub mod cars;
pub mod dds;
pub mod sgc;
// Shared upstream cursor also contains SGS/SGA helpers not selected here.
pub mod ini;
pub mod lbs;
pub mod rbz;
#[allow(dead_code)]
mod sg_framing;
pub mod stage_textures;
