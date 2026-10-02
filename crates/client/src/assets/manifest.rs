use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use serde::Deserialize;

#[derive(Asset, TypePath, Deserialize)]
pub struct Manifest {
    pub metadata: Metadata,
    pub entry_points: HashMap<String, String>,
}

#[derive(Deserialize)]
pub struct Metadata {
    name: String,
    version: String,
}
