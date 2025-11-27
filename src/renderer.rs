use std::collections::BTreeMap;
use std::sync::Arc;

use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, CopyImageInfo,
};
use vulkano::descriptor_set::layout::DescriptorType;
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::device::{
    Device, DeviceCreateInfo, DeviceExtensions, Queue, QueueCreateInfo, QueueFlags,
};
use vulkano::format::{Format, FormatFeatures};
use vulkano::image::view::ImageView;
use vulkano::image::{Image, ImageCreateInfo, ImageLayout, ImageTiling, ImageType, ImageUsage, SampleCount};
use vulkano::instance::{Instance, InstanceCreateFlags, InstanceCreateInfo};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator};
use vulkano::pipeline::layout::{PipelineDescriptorSetLayoutCreateInfo, PipelineLayoutCreateFlags};
use vulkano::pipeline::{Pipeline, PipelineLayout, PipelineShaderStageCreateInfo, ComputePipeline};
use vulkano::pipeline::compute::ComputePipelineCreateInfo;
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::descriptor_set::{DescriptorSet, WriteDescriptorSet};
use vulkano::descriptor_set::layout::{DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo};
use vulkano::swapchain::{self, Surface, Swapchain, SwapchainCreateInfo, SwapchainPresentInfo};
use vulkano::sync::{GpuFuture, Sharing};
use vulkano::{Validated, VulkanError};

use winit::event_loop::ActiveEventLoop;
use winit::window::Window;


pub struct Renderer {
	_instance: Arc<Instance>,
	_device: Arc<Device>,
	queue: Arc<Queue>,
	swapchain: Arc<Swapchain>,
	swapchain_images: Vec<Arc<Image>>,
	cs_images: Vec<Arc<Image>>,
	command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
	compute_pipeline: Arc<ComputePipeline>,
	descriptor_sets: Vec<Arc<DescriptorSet>>,
}

impl Renderer {
	pub fn new(event_loop: &ActiveEventLoop, window: &Arc<Window>) -> Self {
		let library = vulkano::VulkanLibrary::new().expect("no local Vulkan library/DLL");

		let required_extensions = Surface::required_extensions(&event_loop).unwrap();
		let instance = Instance::new(
			library,
			InstanceCreateInfo {
				flags: InstanceCreateFlags::ENUMERATE_PORTABILITY,
				enabled_extensions: required_extensions,
				..Default::default()
			},
		)
		.expect("failed to create instance");

		let surface = Surface::from_window(instance.clone(), window.clone()).unwrap();

		let device_extensions = DeviceExtensions {
			khr_swapchain: true,
			..DeviceExtensions::empty()
		};

		let (physical_device, queue_family_index) = select_physical_device(&instance, &surface, &device_extensions);
		println!(
			"Using device: {} (type: {:?})",
			physical_device.properties().device_name,
			physical_device.properties().device_type
		);

		let (device, mut queues) = Device::new(
			physical_device.clone(),
			DeviceCreateInfo {
				queue_create_infos: vec![QueueCreateInfo {
					queue_family_index,
					..Default::default()
				}],
				enabled_extensions: device_extensions, // new
				..Default::default()
			},
		)
		.expect("failed to create device");

		let queue = queues.next().unwrap();

		let (swapchain, swapchain_images) = {
			let caps = physical_device
				.surface_capabilities(&surface, Default::default())
				.expect("failed to get surface capabilities");

			let dimensions = window.inner_size();
			let composite_alpha = caps.supported_composite_alpha.into_iter().next().unwrap();

			// Find a format that supports storage images and supported by the surface
			let image_format = physical_device
				.surface_formats(&surface, Default::default())
				.unwrap()
				.iter().find(|(format, color)| {
					(physical_device.format_properties(*format).unwrap()
						.format_features(ImageTiling::Optimal, Default::default())
						& FormatFeatures::TRANSFER_DST != FormatFeatures::empty())
					&& *color == vulkano::swapchain::ColorSpace::SrgbNonLinear
				}
				)
				.map(|(format, _)| *format)
				.expect("no supported format found");
				
			println!("Swapchain image format: {:?}", image_format);

			Swapchain::new(
				device.clone(),
				surface,
				SwapchainCreateInfo {
					min_image_count: caps.min_image_count,
					image_format,
					image_extent: dimensions.into(),
					image_usage: ImageUsage::TRANSFER_DST,
					composite_alpha,
					..Default::default()
				},
			)
			.unwrap()
		};

		let memory_allocator = Arc::new(StandardMemoryAllocator::new_default(device.clone()));
		
		let cs_images: Vec<Arc<Image>> = swapchain_images
			.iter()
			.map(|image| {
				Image::new(
					memory_allocator.clone(),
					ImageCreateInfo {
						image_type: ImageType::Dim2d,
						format: Format::R8G8B8A8_UNORM,
						extent: image.extent(),
						mip_levels: 1,
						array_layers: 1,
						samples: SampleCount::Sample1,
						tiling: ImageTiling::Optimal,
						usage: ImageUsage::STORAGE | ImageUsage::TRANSFER_SRC,
						sharing: Sharing::Exclusive,
						initial_layout: ImageLayout::Undefined,
						..Default::default()
					},
					AllocationCreateInfo {
						memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
						..Default::default()
					}
				)
				.unwrap()
			})
			.collect();

		let cs_image_views: Vec<Arc<ImageView>> = cs_images
			.iter()
			.map(|image| ImageView::new_default(image.clone()).unwrap())
			.collect();

		mod cs {
			vulkano_shaders::shader! {
				ty: "compute",
				src: r"
					#version 460

					layout(local_size_x = 8, local_size_y = 8, local_size_z = 1) in;

					layout(set = 0, binding = 0, rgba8) uniform writeonly image2D img;

					void main() {
						vec2 norm_coordinates = (gl_GlobalInvocationID.xy + vec2(0.5)) / vec2(imageSize(img));

						vec2 c = (norm_coordinates - vec2(0.5)) * 2.0 - vec2(1.0, 0.0);

						vec2 z = vec2(0.0, 0.0);
						float i;
						for (i = 0.0; i < 1.0; i += 0.005) {
							z = vec2(
								z.x * z.x - z.y * z.y + c.x,
								z.y * z.x + z.x * z.y + c.y
							);

							if (length(z) > 4.0) {
								break;
							}
						}

						vec4 to_write = vec4(vec3(i), 1.0);
						imageStore(img, ivec2(gl_GlobalInvocationID.xy), to_write);
					}
				",
			}
		}

		let shader = cs::load(device.clone()).expect("failed to create shader module");

		let cs = shader.entry_point("main").unwrap();
		let stage = PipelineShaderStageCreateInfo::new(cs);
		let layout = PipelineLayout::new(
			device.clone(),
			PipelineDescriptorSetLayoutCreateInfo {
				flags: PipelineLayoutCreateFlags::empty(),
				set_layouts: vec![DescriptorSetLayoutCreateInfo {
					bindings: BTreeMap::from([(
						0,
						DescriptorSetLayoutBinding {
							stages: vulkano::shader::ShaderStages::COMPUTE,
							descriptor_type: DescriptorType::StorageImage,
							descriptor_count: 1,
							..DescriptorSetLayoutBinding::descriptor_type(DescriptorType::StorageImage)
						},
					)]),
					..Default::default()
				}],
				push_constant_ranges: vec![],
			}
				.into_pipeline_layout_create_info(device.clone())
				.unwrap(),
		)
		.unwrap();

		let compute_pipeline = ComputePipeline::new(
			device.clone(),
			None,
			ComputePipelineCreateInfo::stage_layout(stage, layout),
		)
		.expect("failed to create compute pipeline");

		let descriptor_set_allocator = Arc::new(StandardDescriptorSetAllocator::new(device.clone(), Default::default()));

		let layout = compute_pipeline.layout().set_layouts().get(0).unwrap();
		let descriptor_sets = (0..cs_image_views.len())
			.enumerate()
			.map(|(i, _)| {
				DescriptorSet::new(
					descriptor_set_allocator.clone(),
					layout.clone(),
					[WriteDescriptorSet::image_view(0, cs_image_views[i].clone())],
					[],
				)
				.unwrap()
			})
			.collect::<Vec<_>>();

		let command_buffer_allocator = Arc::new(StandardCommandBufferAllocator::new(device.clone(), Default::default()));

		Self {
			_instance: instance,
			_device: device,
			queue,
			swapchain,
			swapchain_images,
			cs_images,
			command_buffer_allocator,
			compute_pipeline,
			descriptor_sets,
		}
	}

	pub fn draw(&mut self) {
		// Get the next image from the swapchain
		let (image_i, _suboptimal, acquire_swapchain_image_future) =
			match swapchain::acquire_next_image(self.swapchain.clone(), None)
				.map_err(Validated::unwrap)
			{
				Ok(r) => r,
				Err(VulkanError::OutOfDate) => {
					// recreate_swapchain = true;
					return;
				}
				Err(e) => panic!("failed to acquire next image: {e}"),
			};
		

		let mut cmd_buf_builder = AutoCommandBufferBuilder::primary(
			self.command_buffer_allocator.clone(),
			self.queue.queue_family_index(),
			CommandBufferUsage::OneTimeSubmit,
		)
		.unwrap();

		cmd_buf_builder.bind_pipeline_compute(self.compute_pipeline.clone()).unwrap();
		cmd_buf_builder.bind_descriptor_sets(
			vulkano::pipeline::PipelineBindPoint::Compute,
			self.compute_pipeline.layout().clone(),
			0,
			self.descriptor_sets[image_i as usize].clone(),
		).unwrap();

		unsafe {
			cmd_buf_builder.dispatch(
				[self.swapchain.image_extent()[0] / 8, self.swapchain.image_extent()[1] / 8, 1]
			).unwrap();
		}

		cmd_buf_builder.copy_image(CopyImageInfo::images(
			self.cs_images[image_i as usize].clone(),
			self.swapchain_images[image_i as usize].clone()
		)).unwrap();

		let command_buffer = cmd_buf_builder.build().unwrap();

		let future = acquire_swapchain_image_future
			.then_execute(self.queue.clone(), command_buffer)
			.unwrap()
			.then_swapchain_present(
				self.queue.clone(),
				SwapchainPresentInfo::swapchain_image_index(self.swapchain.clone(), image_i),
			)
			.then_signal_fence_and_flush();

		match future {
			Ok(future) => {
				future.wait(None).unwrap();
			}
			Err(e) => {
				eprintln!("Failed to flush future: {:?}", e);
			}
		}
	}
}

fn select_physical_device(
    instance: &Arc<Instance>,
    surface: &Arc<Surface>,
    device_extensions: &DeviceExtensions,
) -> (Arc<PhysicalDevice>, u32) {
    instance
        .enumerate_physical_devices()
        .expect("failed to enumerate physical devices")
        .filter(|p| p.supported_extensions().contains(device_extensions))
        .filter_map(|p| {
            p.queue_family_properties()
                .iter()
                .enumerate()
                .position(|(i, q)| {
                    q.queue_flags.contains(QueueFlags::GRAPHICS)
                        && p.surface_support(i as u32, surface).unwrap_or(false)
                })
                .map(|q| (p, q as u32))
        })
        .min_by_key(|(p, _)| match p.properties().device_type {
            PhysicalDeviceType::DiscreteGpu => 0,
            PhysicalDeviceType::IntegratedGpu => 1,
            PhysicalDeviceType::VirtualGpu => 2,
            PhysicalDeviceType::Cpu => 3,
            _ => 4,
        })
        .expect("no device available")
}
