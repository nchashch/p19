use bevy::light::Skybox;
use bevy::{
    anti_alias::taa::TemporalAntiAliasing,
    pbr::ScreenSpaceAmbientOcclusion,
    prelude::*,
    render::render_resource::{TextureViewDescriptor, TextureViewDimension},
};

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
