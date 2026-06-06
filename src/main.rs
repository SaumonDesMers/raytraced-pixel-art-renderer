use bevy::{
    asset::RenderAssetUsages,
    mesh::Indices,
    prelude::*,
    render::{
        RenderPlugin,
        settings::{RenderCreation, WgpuFeatures, WgpuSettings},
    },
};
use wgpu_types::{FeaturesWGPU, PrimitiveTopology};

mod render;
use render::{PixelArtRendererPlugin, RaytracingMesh3d};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(RenderPlugin {
                render_creation: RenderCreation::Automatic(WgpuSettings {
                    features: WgpuFeatures {
                        features_wgpu: FeaturesWGPU::EXPERIMENTAL_RAY_QUERY,
                        ..default()
                    },
                    ..default()
                }),
                ..default()
            }),
            bevy_mod_debugdump::CommandLineArgs,
            PixelArtRendererPlugin,
        ))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    let positions = vec![
        [0.0, 1.0, 0.0],   // Top
        [1.0, -1.0, 0.0],  // Bottom Right
        [-1.0, -1.0, 0.0], // Bottom Left
    ];

    let normals = vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]];

    let uvs = vec![[0.5, 0.0], [0.0, 1.0], [1.0, 1.0]];

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);

    mesh.insert_indices(Indices::U32(vec![0, 1, 2]));

	mesh.enable_raytracing = true;

	let mesh_handle = meshes.add(mesh);
    commands.spawn((
        RaytracingMesh3d(mesh_handle.clone()),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
}
