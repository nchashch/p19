use bevy::prelude::*;

/// Named `LevelState` rather than `ServerState` to avoid colliding with `bevy_replicon`'s own
/// `ServerState` (`Running`/`Stopped`, tracking the QUIC listener) — both are in scope together
/// wherever `bevy_replicon::prelude::*` is imported.
#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LevelState {
    #[default]
    Idle,
    LevelLoaded,
}

pub struct LevelStatePlugin;

impl Plugin for LevelStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<LevelState>();
    }
}
