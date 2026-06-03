@group(0) @binding(0) var color_tex: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8, 1)
fn raytrace(
	@builtin(global_invocation_id) coord: vec3<u32>
) {
	let size = textureDimensions(color_tex);
	let color = vec4(f32(coord.x) / f32(size.x), f32(coord.y) / f32(size.y), 0.0, 1.0);
	textureStore(color_tex, coord.xy, color);
}
