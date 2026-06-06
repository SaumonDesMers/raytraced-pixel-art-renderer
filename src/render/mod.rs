use std::borrow::Cow;

use bevy::{
    asset::RenderAssetUsages,
    core_pipeline::core_2d::graph::{Core2d, Node2d::StartMainPass},
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    mesh::PrimitiveTopology,
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup,
        RenderSystems::{self, PrepareBindGroups},
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        mesh::{
            RenderMesh,
            allocator::{MeshAllocator, allocate_and_free_meshes},
        },
        render_asset::{RenderAssets, prepare_assets},
        render_graph::{
            NodeRunError, RenderGraph, RenderGraphContext, RenderLabel, ViewNode, ViewNodeRunner,
        },
        render_resource::{
            AccelerationStructureFlags, AccelerationStructureUpdateMode, AsBindGroup, BindGroup,
            BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, BufferUsages,
            CachedComputePipelineId, ComputePassDescriptor, ComputePipelineDescriptor,
            CreateTlasDescriptor, Extent3d, PipelineCache, ShaderStages, StorageTextureAccess,
            TextureDimension, TextureFormat, TextureUsages, TlasInstance,
            binding_types::{acceleration_structure, texture_storage_2d},
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
        sync_world::SyncToRenderWorld,
        texture::GpuImage,
        view::ViewTarget,
    },
    shader::ShaderRef,
    sprite_render::{Material2d, Material2dPlugin},
};

mod blas;
use blas::BlasManager;
use wgpu_types::CommandEncoderDescriptor;

use crate::render::blas::{
    compact_raytracing_blas, extract_raytracing_scene, prepare_raytracing_blas,
};

#[derive(Component, Clone)]
#[require(Transform, SyncToRenderWorld)]
pub struct RaytracingMesh3d(pub Handle<Mesh>);

pub struct PixelArtRendererPlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct PixelArtRendererLabel;

impl Plugin for PixelArtRendererPlugin {
    fn build(&self, _app: &mut App) {}

    fn finish(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            // .add_systems(Update, resize_images)
            .add_plugins((
                ExtractResourcePlugin::<PixelArtRendererImages>::default(),
                Material2dPlugin::<Fullscreen>::default(),
            ));

        let render_app = app.sub_app_mut(RenderApp);

        render_app
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(Render, prepare_bind_groups.in_set(PrepareBindGroups));

        let view_node_runner =
            ViewNodeRunner::new(RaytracingNode::default(), render_app.world_mut());

        let mut render_graph = render_app.world_mut().resource_mut::<RenderGraph>();

        let sub_graph = render_graph.sub_graph_mut(Core2d);
        sub_graph.add_node(PixelArtRendererLabel, view_node_runner);
        sub_graph.add_node_edge(PixelArtRendererLabel, StartMainPass);

        render_app
            .world_mut()
            .resource_mut::<MeshAllocator>()
            .extra_buffer_usages |= BufferUsages::BLAS_INPUT | BufferUsages::STORAGE;

        render_app
            .init_resource::<BlasManager>()
            .add_systems(ExtractSchedule, extract_raytracing_scene)
            .add_systems(
                Render,
                (
                    prepare_raytracing_blas
                        .in_set(RenderSystems::PrepareAssets)
                        .before(prepare_assets::<RenderMesh>)
                        .after(allocate_and_free_meshes),
                    compact_raytracing_blas
                        .in_set(RenderSystems::PrepareAssets)
                        .after(prepare_raytracing_blas),
                ),
            );
    }
}

fn setup(
    mut commands: Commands,
    images: ResMut<Assets<Image>>,
    window: Single<&Window>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<Fullscreen>>,
) {
    let size = Extent3d {
        width: window.width() as u32,
        height: window.height() as u32,
        depth_or_array_layers: 1,
    };
    let images = PixelArtRendererImages::new(images, size);
    commands.insert_resource(images.clone());

    commands.spawn((
        Mesh2d(meshes.add(fullscreen_mesh(size.width as f32, size.height as f32))),
        MeshMaterial2d(materials.add(Fullscreen {
            color: images.color[0].clone(),
        })),
    ));
    commands.spawn((Camera2d::default(),));
}

fn fullscreen_mesh(width: f32, height: f32) -> Mesh {
    let (width, height) = (width / 2.0, height / 2.0);
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            [-width, height, 0.0],
            [width * 3.0, height, 0.0],
            [-width, -height * 3.0, 0.0],
        ],
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![[0.0, 0.0], [2.0, 0.0], [0.0, 2.0]],
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_COLOR,
        vec![
            [0.0, 0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0, 1.0],
        ],
    )
}

#[derive(AsBindGroup, Clone, Asset, TypePath, Default)]
struct Fullscreen {
    #[texture(0, visibility(fragment))]
    #[sampler(1, visibility(fragment))]
    color: Handle<Image>,
}

impl Material2d for Fullscreen {
    fn fragment_shader() -> ShaderRef {
        "shaders/fullscreen.wgsl".into()
    }
}

#[derive(Resource, Clone, ExtractResource)]
struct PixelArtRendererImages {
    color: [Handle<Image>; 2],
}

impl PixelArtRendererImages {
    fn new(mut images: ResMut<Assets<Image>>, size: Extent3d) -> Self {
        let mut color = Image::new_fill(
            size,
            TextureDimension::D2,
            &[0, 255, 0, 255],
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::RENDER_WORLD,
        );
        color.texture_descriptor.usage |= TextureUsages::STORAGE_BINDING;
        color.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::ClampToEdge,
            address_mode_v: ImageAddressMode::ClampToEdge,
            mag_filter: ImageFilterMode::Nearest,
            min_filter: ImageFilterMode::Nearest,
            mipmap_filter: ImageFilterMode::Nearest,
            ..default()
        });

        Self {
            color: [images.add(color.clone()), images.add(color)],
        }
    }
}

#[derive(Resource)]
struct RaytracingBindGroups {
    raytracing: BindGroup,
}

fn tlas_transform(transform: &Mat4) -> [f32; 12] {
    transform.transpose().to_cols_array()[..12]
        .try_into()
        .unwrap()
}

fn prepare_bind_groups(
    mut commands: Commands,
    pipeline: Res<RaytracingPipeline>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    pixel_art_images: Res<PixelArtRendererImages>,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    instances_query: Query<(Entity, &RaytracingMesh3d, &GlobalTransform)>,
    blas_manager: Res<BlasManager>,
    render_queue: Res<RenderQueue>,
) {
    let color_view = gpu_images.get(&pixel_art_images.color[0]).unwrap();

    let mut tlas = render_device
        .wgpu_device()
        .create_tlas(&CreateTlasDescriptor {
            label: Some("tlas"),
            flags: AccelerationStructureFlags::PREFER_FAST_TRACE,
            update_mode: AccelerationStructureUpdateMode::Build,
            max_instances: instances_query.iter().len() as u32,
        });

    let mut instance_id = 0;
    for (_entity, RaytracingMesh3d(mesh_handle), transform) in instances_query.iter() {
        let Some(blas) = blas_manager.get(&mesh_handle.id()) else {
            continue;
        };

        let transform = transform.to_matrix();
        *tlas.get_mut_single(instance_id).unwrap() = Some(TlasInstance::new(
            blas,
            tlas_transform(&transform),
            Default::default(),
            0xFF,
        ));

        instance_id += 1;
    }
	// info!("Building TLAS with {} instances", instance_id);

    let mut command_encoder = render_device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("build_tlas_command_encoder"),
    });
    command_encoder.build_acceleration_structures(&[], [&tlas]);
    render_queue.submit([command_encoder.finish()]);

    let bind_group = render_device.create_bind_group(
        None,
        &pipeline_cache.get_bind_group_layout(&pipeline.bind_group_layout),
        &BindGroupEntries::sequential((&color_view.texture_view, tlas.as_binding())),
    );
    commands.insert_resource(RaytracingBindGroups {
        raytracing: bind_group,
    });
}

#[derive(Resource)]
struct RaytracingPipeline {
    bind_group_layout: BindGroupLayoutDescriptor,
    pipeline: CachedComputePipelineId,
}

fn init_pipeline(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
) {
    let bind_group_layout = BindGroupLayoutDescriptor::new(
        "RaytracingImages",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                texture_storage_2d(TextureFormat::Rgba8Unorm, StorageTextureAccess::WriteOnly),
                acceleration_structure(),
            ),
        ),
    );

    let shader = asset_server.load("shaders/raytracing.wgsl");
    let pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        layout: vec![bind_group_layout.clone()],
        shader,
        entry_point: Some(Cow::from("raytrace")),
        ..default()
    });

    commands.insert_resource(RaytracingPipeline {
        bind_group_layout,
        pipeline,
    });
}

#[derive(Default)]
struct RaytracingNode;

impl ViewNode for RaytracingNode {
    type ViewQuery = (&'static ViewTarget,);

    fn run<'w>(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        view_query: bevy::ecs::query::QueryItem<'w, '_, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let bind_group = &world.resource::<RaytracingBindGroups>().raytracing;
        let pipeline_cache = &world.resource::<PipelineCache>();
        let pipeline = &world.resource::<RaytracingPipeline>();
        let size = view_query.0.main_texture().size();

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
