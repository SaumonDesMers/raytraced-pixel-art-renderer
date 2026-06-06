// enable wgpu_ray_query;

@group(0) @binding(0) var color_tex: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(1) var tlas: acceleration_structure;

const fov = 90.0;

@compute @workgroup_size(8, 8, 1)
fn raytrace(
	@builtin(global_invocation_id) coord: vec3<u32>
) {
	let size = textureDimensions(color_tex);

	let ray_origin = vec3<f32>(0.0, 0.0, 1.0);
	// let ray_direction = normalize(pixel_world_position(coord.xy, size) - ray_origin);
	let ray_direction = vec3<f32>(0.0, 0.0, -1.0);

	let ray = RayDesc(0, 0xFF, 0.01, 100.0, ray_origin, ray_direction);
	var ray_query: ray_query;
	rayQueryInitialize(&ray_query, tlas, ray);
	while rayQueryProceed(&ray_query) {}
	let ray_intersection = rayQueryGetCommittedIntersection(&ray_query);

	// var color = vec4(abs(ray_direction), 1.0);
	var color = vec4(f32(coord.x) / f32(size.x), f32(coord.y) / f32(size.y), 0.0, 1.0);
	// var color = vec4(0.0, 0.0, 0.0, 1.0);

	if (ray_intersection.kind == RAY_QUERY_INTERSECTION_TRIANGLE) {
		color = vec4(1.0, 1.0, 1.0, 1.0);
	}

	textureStore(color_tex, coord.xy, color);
}

fn pixel_world_position(coord: vec2<u32>, size: vec2<u32>) -> vec3<f32> {
	let ndc = vec2<f32>(
		(f32(coord.x) + 0.5) / f32(size.x) * 2.0 - 1.0,
		1.0 - (f32(coord.y) + 0.5) / f32(size.y) * 2.0
	);
	let aspect_ratio = f32(size.x) / f32(size.y);
	let fov_adjustment = tan(radians(fov) / 2.0);
	let ray_dir = vec3<f32>(ndc.x * aspect_ratio * fov_adjustment, ndc.y * fov_adjustment, -1.0);
	return ray_dir;
}
