//! Types and simulation logic shared between `client` and `server`.
//!
//! This is scaffolding for the eventual simulation/presentation split — content moves here
//! incrementally as that split happens, not all at once. `combat` is the first real occupant.

pub mod character_controller;
pub mod combat;
pub mod cube_spawner;
pub mod npc_spawner;
