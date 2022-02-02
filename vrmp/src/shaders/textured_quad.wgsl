{% include "inc_common.wgsl" %}

[[group(1), binding(0)]]
var vscreen_tex: texture_2d<f32>;

struct VertexInput {
  [[location(0)]] position: vec3<f32>;
  [[location(1)]] texcoord: vec2<f32>;
};

struct VertexOutput {
  [[builtin(position)]] position: vec4<f32>;
  [[location(0)]] texcoord: vec2<f32>;
};

struct PushConstants {
  model: mat4x4<f32>;
};

var<push_constant> push: PushConstants;

[[stage(vertex)]]
fn vs_main(in: VertexInput) -> VertexOutput {
  var out: VertexOutput;
  out.position = (camera_state.mvp * push.model) * vec4<f32>(in.position, 1.0);
  out.texcoord = in.texcoord;
  return out;
}

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
  var tc = in.texcoord;
  tc.y = 1.0 - tc.y;
  return textureSample(vscreen_tex, sampler_tex, tc);
}