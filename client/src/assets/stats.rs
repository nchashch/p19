use bevy::prelude::*;
use serde::Deserialize;

#[derive(Asset, TypePath, Deserialize)]
pub struct Stats {
    pub vitality: u32,
    pub strength: u32,
    pub agility: u32,
    pub intellect: u32,
    pub luck: u32,
    pub fire_immunity: bool,
    // ... etc, etc
}
