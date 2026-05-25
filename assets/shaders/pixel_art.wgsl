@group(0) @binding(0) var color_tex: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8, 1)
fn raytrace(
	@builtin(global_invocation_id) invocation_id: vec3<u32>
) {
	let coord = vec2<i32>(i32(invocation_id.x), i32(invocation_id.y));
	let color = vec4(1.0, 0.0, 0.0, 1.0);
	textureStore(color_tex, coord, color);
}
