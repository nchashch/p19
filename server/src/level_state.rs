use bevy::asset::AssetPath;
use bevy::prelude::*;

/// Named `LevelState` rather than `ServerState` to avoid colliding with `bevy_replicon`'s own
/// `ServerState` (`Running`/`Stopped`, tracking the QUIC listener) — both are in scope together
/// wherever `bevy_replicon::prelude::*` is imported.
///
/// `Loading`/`LevelLoaded` carry the requested level's id so `on_load_level_request` can reject
/// a second `LoadLevelRequest` arriving while one is already in flight or already loaded — e.g. a
/// client sending it twice (double-fired UI button, retried packet, or a malicious client) used
/// to spawn a second `InGameRoot`/`WorldAssetRoot` into the same ECS world, corrupting the game
/// (duplicate colliders, duplicate `PlayerCharacterSpawner`s, etc.). Switching to a genuinely
/// different level isn't supported yet either way — the server doesn't despawn a previous level's
/// geometry on reload (see `CLAUDE.md`'s "known gap" on this) — so any request beyond the first is
/// rejected regardless of id until that's addressed.
///
/// Deliberately a plain `Resource`, not a Bevy `States` type: `bevy_replicon` can trigger
/// `on_load_level_request` twice in the same frame (both `LoadLevelRequest` messages already sat
/// in the same network batch), and `States`/`NextState` only applies a transition once per frame
/// via the dedicated `StateTransition` schedule — a `next_state.set(...)` in the first call
/// wouldn't be visible to `Res<State<_>>` in the second call until next frame, so the dedup check
/// would silently do nothing against the exact back-to-back-in-one-frame case it exists for. A
/// plain resource mutated directly makes the check-and-set atomic within one observer call, so
/// the very next call in the same frame sees it immediately.
#[derive(Resource, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum LevelState {
    #[default]
    Idle,
    Loading(AssetPath<'static>),
    LevelLoaded(AssetPath<'static>),
}

pub struct LevelStatePlugin;

impl Plugin for LevelStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LevelState>();
    }
}
