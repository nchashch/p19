use bevy::prelude::*;
use shared::combat::SharedCombatPlugin;

fn main() {
    App::new()
        .add_plugins((MinimalPlugins, SharedCombatPlugin))
        .run();
}
