struct CameraState {
  // despite the name includes only projection and view component
  // "view" component doesn't include world origin
  // for 3d things "model" should be supplied via push constant and world origin needs to be included into it
  // not used for fullscreen projection-based rendering
  // TODO: rename it
  mvp: mat4x4<f32>;
  inverse_projection: mat4x4<f32>;
  // used for fullscreen projection-based rendering
  // IMPORTANT: includes world origin orientation
  view_orientation: mat4x4<f32>;
  // 0 - left
  // 1 - right
  eye_index: u32;
  // 0 - mono
  // 1 - left/right
  // 2 - top/bottom
  mode: u32;
  // in radians, eye-based sign is already applied
  stereo_adjust: f32;
  shader_debug: f32;
};

[[group(0), binding(0)]]
var<uniform> camera_state: CameraState;

[[group(0), binding(1)]]
var sampler_tex: sampler;
