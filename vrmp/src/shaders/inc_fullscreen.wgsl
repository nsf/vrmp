[[group(1), binding(0)]]
var shared_tex: texture_2d<f32>;

struct VertexOutput {
  [[builtin(position)]] position: vec4<f32>;
  [[location(0)]] inv_pos: vec3<f32>;
};

[[stage(vertex)]]
fn vs_main([[builtin(vertex_index)]] in_vertex_index: u32) -> VertexOutput {
  var v = vec2<f32>(-1.0, -1.0);
  if (in_vertex_index == 1u) {
    v = vec2<f32>(3.0, -1.0);
  } else if (in_vertex_index == 2u) {
    v = vec2<f32>(-1.0, 3.0);
  }
  var out: VertexOutput;
  out.position = vec4<f32>(v, 0.0, 1.0);

  let p = camera_state.inverse_projection * vec4<f32>(v, 0.0, 1.0);
  out.inv_pos = p.xyz / p.w;
  return out;
}
