use bevy::light::Skybox;
use bevy::{
    anti_alias::taa::TemporalAntiAliasing,
    pbr::{DistanceFog, FogFalloff, ScreenSpaceAmbientOcclusion},
    prelude::*,
};
use bevy_mod_xr::camera::XrCamera;

/// Matches `main.rs`'s `ClearColor` — a fallback background for whatever the skybox doesn't
/// cover (there's always a moment before `skyboxes/night_sky.ktx2` finishes streaming in where
/// nothing is rendered there yet), so fog blends into that instead of a visibly different flat
/// color in the meantime.
const FOG_COLOR: Color = Color::srgb(0.1, 0.1, 0.15);
/// Chosen relative to existing gameplay distances rather than arbitrarily: `targeting::SELECT_RANGE`
/// and `nameplate.rs`'s `FADE_END_DISTANCE` (30.0) are both well inside `FOG_START`, so fog never
/// visibly interferes with targeting or nameplate readability at any range they actually matter.
const FOG_START: f32 = 40.0;
const FOG_END: f32 = 180.0;

/// Scales `skyboxes/night_sky.ktx2`'s stored HDR values (roughly 0.006-2.0, per the source HDRI —
/// see `scripts/hdri_to_skybox.py`) into the candela-per-square-meter units `Skybox::brightness`
/// expects. Tuned by eye for a dim-but-visible night sky against this project's existing lighting
/// (`main.rs`'s `GlobalAmbientLight`); adjust here if the sky reads too bright/dark after other
/// lighting changes.
const SKYBOX_BRIGHTNESS: f32 = 100.0;

fn distance_fog() -> DistanceFog {
    DistanceFog {
        color: FOG_COLOR,
        falloff: FogFalloff::Linear {
            start: FOG_START,
            end: FOG_END,
        },
        ..default()
    }
}

fn skybox(asset_server: &AssetServer) -> Skybox {
    // Unlike the old PNG-vertical-strip cubemap this replaces, a KTX2 file carries its own
    // cubemap metadata (see `scripts/hdri_to_skybox.py`'s doc comment) — Bevy's KTX2 loader
    // detects the 6 faces and sets up `TextureViewDimension::Cube` automatically, so this can
    // just be loaded and inserted directly, with no manual "wait for load, then reinterpret
    // the texture" dance required (that machinery lived here before; see git history).
    Skybox {
        image: Some(asset_server.load("skyboxes/night_sky_clean_bc6h.ktx2")),
        brightness: SKYBOX_BRIGHTNESS,
        ..default()
    }
}

pub fn player_camera(asset_server: &Res<AssetServer>) -> impl Bundle {
    (
        Camera3d::default(),
        IsDefaultUiCamera,
        Msaa::Off,
        TemporalAntiAliasing::default(),
        ScreenSpaceAmbientOcclusion::default(),
        distance_fog(),
        skybox(asset_server),
    )
}

pub struct PlayerCameraPlugin;

impl Plugin for PlayerCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_xr_camera_added);
    }
}

/// In VR, rendering goes through separate `XrCamera` entities (one per eye — spawned by
/// `bevy_openxr`'s view-setup code with only a bare default `Camera3d`, entirely independent of
/// the `Camera3d` `player_camera()` spawns), so `Skybox`/`DistanceFog`/etc. on our own camera
/// never reach them. An `On<Add, XrCamera>` observer rather than a polling system, per this
/// project's component-lifecycle convention — these entities are created at a time relative to
/// session startup that nothing else here controls (see `camera.rs`'s git history / CLAUDE.md
/// for the broader pattern of XR entities appearing later than `Startup`/`OnEnter`).
fn on_xr_camera_added(
    added: On<Add, XrCamera>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands
        .entity(added.entity)
        .insert((distance_fog(), skybox(&asset_server)));
}
