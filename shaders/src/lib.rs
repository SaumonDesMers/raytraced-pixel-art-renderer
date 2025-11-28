#![no_std]

use spirv_std::{
	glam::{Vec2, IVec2, UVec2, UVec3, Vec4},
	image::Image,
	spirv,
};

#[spirv(compute(threads(8, 8, 1)))]
pub fn main_cs(
	#[spirv(global_invocation_id)] global_id: UVec3,
	#[spirv(descriptor_set = 0, binding = 0)] img: &Image!(2D, format = rgba8)
) {
	let norm_coordinates = Vec2::new(global_id.x as f32 + 0.5, global_id.y as f32 + 0.5) / img.query_size::<UVec2>().as_vec2();

	let c = (norm_coordinates - Vec2::splat(0.5)) * 2.0 - Vec2::new(1.0, 0.0);

	let mut z = Vec2::new(0.0, 0.0);
	let mut i = 0.0;
	while i < 1.0 {
		z = Vec2::new(
			z.x * z.x - z.y * z.y + c.x,
			z.y * z.x + z.x * z.y + c.y
		);

			if z.length() > 4.0 {
				break;
			}

		i += 0.005;
	}

	let to_write = Vec4::new(i, i, i, 1.0);
	unsafe {
		img.write(IVec2::new(global_id.x as i32, global_id.y as i32), to_write);
	}
}