use bevy::{prelude::*, render::{RenderPlugin, settings::{RenderCreation, WgpuFeatures, WgpuSettings}}};
use wgpu_types::FeaturesWGPU;

mod render;
use render::PixelArtRendererPlugin;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins
				.set(RenderPlugin {
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
        .run();
}
