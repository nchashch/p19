use bevy::light::Skybox;
use bevy::{
    anti_alias::taa::TemporalAntiAliasing,
    pbr::{DistanceFog, FogFalloff, ScreenSpaceAmbientOcclusion},
    prelude::*,
    render::render_resource::{TextureViewDescriptor, TextureViewDimension},
};

/// Matches `main.rs`'s `ClearColor` — with the skybox disabled (see `player_camera`), the
/// background behind un-fogged geometry *is* the clear color, so fog needs the same tint to blend
/// into it instead of fading distant geometry to a visibly different flat color.
const FOG_COLOR: Color = Color::srgb(0.1, 0.1, 0.15);
/// Chosen relative to existing gameplay distances rather than arbitrarily: `targeting::SELECT_RANGE`
/// and `nameplate.rs`'s `FADE_END_DISTANCE` (30.0) are both well inside `FOG_START`, so fog never
/// visibly interferes with targeting or nameplate readability at any range they actually matter.
const FOG_START: f32 = 40.0;
const FOG_END: f32 = 180.0;

pub struct PlayerCameraPlugin;

impl Plugin for PlayerCameraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Cubemap {
            is_loaded: false,
            image_handle: None,
        });
        app.add_systems(Update, cubemap_loaded);
    }
}

pub fn player_camera(asset_server: &Res<AssetServer>, cubemap: &mut Cubemap) -> impl Bundle {
    let skybox_handle = asset_server.load("Ryfjallet_cubemap.png");
    cubemap.is_loaded = false;
    cubemap.image_handle = Some(skybox_handle.clone());
    (
        Camera3d::default(),
        IsDefaultUiCamera,
        Msaa::Off,
        TemporalAntiAliasing::default(),
        ScreenSpaceAmbientOcclusion::default(),
        DistanceFog {
            color: FOG_COLOR,
            falloff: FogFalloff::Linear {
                start: FOG_START,
                end: FOG_END,
            },
            ..default()
        },
        /*
                Skybox {
                    image: Some(skybox_handle.clone()),
                    brightness: 1000.0,
                    ..default()
                },
        */
    )
}

#[derive(Resource)]
pub struct Cubemap {
    is_loaded: bool,
    image_handle: Option<Handle<Image>>,
}

fn cubemap_loaded(
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut cubemap: ResMut<Cubemap>,
    mut skyboxes: Query<&mut Skybox>,
) {
    if cubemap.image_handle.is_none() {
        return;
    }
    if !cubemap.is_loaded
        && asset_server
            .load_state(&cubemap.image_handle.clone().unwrap())
            .is_loaded()
    {
        let mut image = images
            .get_mut(&cubemap.image_handle.clone().unwrap())
            .unwrap();
        // NOTE: PNGs do not have any metadata that could indicate they contain a cubemap texture,
        // so they appear as one texture. The following code reconfigures the texture as necessary.
        if image.texture_descriptor.array_layer_count() == 1 {
            let layers = image.height() / image.width();
            image
                .reinterpret_stacked_2d_as_array(layers)
                .expect("asset should be 2d texture and height will always be evenly divisible with the given layers");
            image.texture_view_descriptor = Some(TextureViewDescriptor {
                dimension: Some(TextureViewDimension::Cube),
                ..default()
            });
        }
        for mut skybox in &mut skyboxes {
            skybox.image = cubemap.image_handle.clone();
        }
        cubemap.is_loaded = true;
    }
}
