{% include "inc_common.wgsl" %}
{% include "inc_util.wgsl" %}

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
  let sadjust = mat4x4<f32>(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    -camera_state.stereo_adjust, 0.0, 0.0, 1.0,
  );
  var out: VertexOutput;
  out.position = (camera_state.mvp * sadjust * push.model) * vec4<f32>(in.position, 1.0);
  out.texcoord = in.texcoord;
  return out;
}

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
  var tc = in.texcoord;
  tc.y = 1.0 - tc.y;
  tc = stereo(tc);
  return textureSample(vscreen_tex, sampler_tex, tc);
}