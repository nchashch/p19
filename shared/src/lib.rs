//! Types and simulation logic shared between `client` and `server`.
//!
//! This is scaffolding for the eventual simulation/presentation split — content moves here
//! incrementally as that split happens, not all at once. `combat` is the first real occupant.

pub mod assets;
pub mod character_controller;
pub mod client_events;
pub mod combat;
pub mod cube_spawner;
pub mod game_state;
pub mod level;
pub mod npc_spawner;
pub mod player;
pub mod replication;
pub mod server_events;
