use std::borrow::Cow;

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::{
        Extract, Render, RenderApp, RenderStartup,
        RenderSystems::PrepareBindGroups,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        graph::CameraDriverLabel,
        render_asset::RenderAssets,
        render_graph::{Node, NodeRunError, RenderGraph, RenderGraphContext, RenderLabel},
        render_resource::{
            BindGroup, BindGroupEntries, BindGroupLayout, BindGroupLayoutEntries,
            CachedComputePipelineId, ComputePassDescriptor, ComputePipelineDescriptor, Extent3d,
            PipelineCache, ShaderStages, StorageTextureAccess, TextureDimension, TextureFormat,
            TextureUsages, binding_types::texture_storage_2d,
        },
        renderer::{RenderContext, RenderDevice},
        texture::GpuImage,
    },
    window::{PrimaryWindow, WindowResized},
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(PixelArtRendererPlugin)
        .run();
}

struct PixelArtRendererPlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct PixelArtRendererLabel;

impl Plugin for PixelArtRendererPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, resize_images)
            .add_plugins(ExtractResourcePlugin::<PixelArtRendererImages>::default());

        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(ExtractSchedule, extract_window_size)
            .add_systems(Render, prepare_bind_groups.in_set(PrepareBindGroups));

        let mut render_graph = render_app.world_mut().resource_mut::<RenderGraph>();

        render_graph.add_node(PixelArtRendererLabel, PixelArtRendererNode::default());
        render_graph.add_node_edge(PixelArtRendererLabel, CameraDriverLabel);
    }
}

fn setup(mut commands: Commands, images: ResMut<Assets<Image>>, window: Single<&Window>) {
    let size = Extent3d {
        width: window.width() as u32,
        height: window.height() as u32,
        depth_or_array_layers: 1,
    };
    commands.insert_resource(PixelArtRendererImages::new(images, size));
}

fn resize_images(
    mut commands: Commands,
    images: ResMut<Assets<Image>>,
    mut resize_reader: MessageReader<WindowResized>,
) {
    if let Some(resize) = resize_reader.read().last() {
        let size = Extent3d {
            width: resize.width as u32,
            height: resize.height as u32,
            depth_or_array_layers: 1,
        };
        commands.insert_resource(PixelArtRendererImages::new(images, size));
    }
}

/// The textures target for the pixel art renderer.
#[derive(Resource, Clone, ExtractResource)]
struct PixelArtRendererImages {
    color: [Handle<Image>; 2],
}

impl PixelArtRendererImages {
    fn new(mut images: ResMut<Assets<Image>>, size: Extent3d) -> Self {
        let mut color = Image::new_fill(
            size,
            TextureDimension::D2,
            &[0, 0, 0, 255],
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::RENDER_WORLD,
        );
        color.texture_descriptor.usage |= TextureUsages::STORAGE_BINDING;
        Self {
            color: [images.add(color.clone()), images.add(color)],
        }
    }
}

#[derive(Resource, Clone, Copy, Default, ExtractResource)]
struct ExtractedWindowSize {
    width: u32,
    height: u32,
}

fn extract_window_size(
    mut commands: Commands,
    windows: Extract<Query<&Window, With<PrimaryWindow>>>,
) {
    if let Ok(window) = windows.single() {
        commands.insert_resource(ExtractedWindowSize {
            width: window.size().x as u32,
            height: window.size().y as u32,
        });
    }
}

#[derive(Resource)]
struct PixelArtRendererBindGroups {
    raytracing: BindGroup,
}

fn prepare_bind_groups(
    mut commands: Commands,
    pipeline: Res<PixelArtRendererPipeline>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    pixel_art_images: Res<PixelArtRendererImages>,
    render_device: Res<RenderDevice>,
) {
    let color_view = gpu_images.get(&pixel_art_images.color[0]).unwrap();

    let bind_group = render_device.create_bind_group(
        None,
        &pipeline.bind_group_layout,
        &BindGroupEntries::sequential((&color_view.texture_view,)),
    );
    commands.insert_resource(PixelArtRendererBindGroups {
        raytracing: bind_group,
    });
}

#[derive(Resource)]
struct PixelArtRendererPipeline {
    bind_group_layout: BindGroupLayout,
    pipeline: CachedComputePipelineId,
}

fn init_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
) {
    let bind_group_layout = render_device.create_bind_group_layout(
        "PixelArtImages",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (texture_storage_2d(
                TextureFormat::Rgba8Unorm,
                StorageTextureAccess::WriteOnly,
            ),),
        ),
    );

    let shader = asset_server.load("shaders/pixel_art.wgsl");
    let pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        layout: vec![bind_group_layout.clone()],
        shader,
        entry_point: Some(Cow::from("raytrace")),
        ..default()
    });

    commands.insert_resource(PixelArtRendererPipeline {
        bind_group_layout,
        pipeline,
    });
}

struct PixelArtRendererNode;

impl Default for PixelArtRendererNode {
    fn default() -> Self {
        Self {}
    }
}

impl Node for PixelArtRendererNode {
    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let bind_group = &world.resource::<PixelArtRendererBindGroups>().raytracing;
        let pipeline_cache = &world.resource::<PipelineCache>();
        let pipeline = &world.resource::<PixelArtRendererPipeline>();
        let size = &world.resource::<ExtractedWindowSize>();

        let mut pass = render_context
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor::default());

        let Some(pipeline) = pipeline_cache.get_compute_pipeline(pipeline.pipeline) else {
            return Ok(());
        };
        pass.set_bind_group(0, bind_group, &[]);
        pass.set_pipeline(pipeline);
        pass.dispatch_workgroups(size.width / 8, size.height / 8, 1);

        Ok(())
    }
}
