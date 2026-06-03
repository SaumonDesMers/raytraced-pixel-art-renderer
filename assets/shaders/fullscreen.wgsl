#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var color_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var color_sampler: sampler;

@fragment
fn fragment(
	mesh: VertexOutput,
) -> @location(0) vec4<f32> {
	// return vec4(fract(mesh.uv), 0.0, 1.0);
	return textureSample(color_texture, color_sampler, mesh.uv);
}
