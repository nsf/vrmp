{% include "inc_common.wgsl" %}
{% include "inc_util.wgsl" %}
{% include "inc_fullscreen.wgsl" %}

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
  let m = mat4_to_mat3(camera_state.view_orientation);
  let ws = m * normalize(in.inv_pos);
  let sc = ws_to_spherical_coords(ws);
  let uv = equirectangular_360(sc);
  let tex = textureSample(shared_tex, sampler_tex, uv);
  return vec4<f32>(tex.rgb, 1.0);
}