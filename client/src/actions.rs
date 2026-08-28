use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;

#[derive(InputAction)]
#[action_output(bool)]
pub(crate) struct AttackAction;

#[derive(InputAction)]
#[action_output(bool)]
pub(crate) struct KillAction;

#[derive(InputAction)]
#[action_output(bool)]
pub(crate) struct Select;

#[derive(InputAction)]
#[action_output(bool)]
pub(crate) struct Deselect;

#[derive(InputAction)]
#[action_output(Vec2)]
pub struct FpsCameraRotation;

#[derive(InputAction)]
#[action_output(Vec2)]
pub struct Movement;

#[derive(InputAction)]
#[action_output(bool)]
pub struct Jump;

#[derive(InputAction)]
#[action_output(bool)]
pub struct MainMenu;

#[derive(InputAction)]
#[action_output(bool)]
pub struct Shoot;

#[derive(InputAction)]
#[action_output(bool)]
pub struct SpawnNpcAction;
