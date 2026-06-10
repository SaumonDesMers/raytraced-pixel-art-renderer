// enable wgpu_ray_query;

@group(0) @binding(0) var color_tex: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(1) var tlas: acceleration_structure;

const fov = 90.0;

@compute @workgroup_size(8, 8, 1)
fn raytrace(
	@builtin(global_invocation_id) coord: vec3<u32>
) {
	let size = textureDimensions(color_tex);
	var color = vec4(0.0, 0.0, 0.0, 1.0);

	let ray_origin = vec3<f32>(0.0, 0.0, 2.0);
	let ray_direction = pixel_world_position(coord.xy, size);

	{
		let ray = RayDesc(0, 0xFF, 0.01, 100.0, ray_origin, ray_direction);
		var ray_query: ray_query;
		rayQueryInitialize(&ray_query, tlas, ray);
		while rayQueryProceed(&ray_query) {}
		let ray_intersection = rayQueryGetCommittedIntersection(&ray_query);

		if (ray_intersection.kind == RAY_QUERY_INTERSECTION_TRIANGLE) {
			color.r = 1.0;
		}
	}
	{
		let ray = Ray(ray_origin, ray_direction, 0.01, 100.0);
		let v0 = vec3<f32>(0.0, 1.0, 0.0);
		let v1 = vec3<f32>(1.0, -1.0, 0.0);
		let v2 = vec3<f32>(-1.0, -1.0, 0.0);
		let ray_intersection = rayTriangleIntersection(ray, v0, v1, v2);

		if (ray_intersection.hit) {
			color.g = 1.0;
		}
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
	return normalize(ray_dir);
}


struct Ray {
	origin: vec3<f32>,
	direction: vec3<f32>,
	t_min: f32,
	t_max: f32,
}

struct MyRayIntersection {
	hit: bool,
	t: f32,
	uv: vec2<f32>,
} 

fn rayTriangleIntersection(
	ray: Ray,
	v0: vec3<f32>,
	v1: vec3<f32>,
	v2: vec3<f32>
) -> MyRayIntersection {
	let e0 = v0 - v2;
	let e1 = v1 - v2;
	let pvec = cross(ray.direction, e1);
	let det = dot(e0, pvec);
	if abs(det) < 0.0001 {
		return MyRayIntersection(false, 0.0, vec2(0.0));
	}
	let inv_det = 1.0 / det;
	let tvec = ray.origin - v2;
	let u = dot(tvec, pvec) * inv_det;
	if u < 0.0 || u > 1.0 {
		return MyRayIntersection(false, 0.0, vec2(0.0));
	}
	let qvec = cross(tvec, e0);
	let v = dot(ray.direction, qvec) * inv_det;
	if v < 0.0 || u + v > 1.0 {
		return MyRayIntersection(false, 0.0, vec2(0.0));
	}
	let t = dot(e1, qvec) * inv_det;
	return MyRayIntersection(true, t, vec2(u, v));
}
