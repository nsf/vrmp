
let PI: f32 = 3.1415926538;

fn mat4_to_mat3(m: mat4x4<f32>) -> mat3x3<f32> {
  return mat3x3<f32>(m[0].xyz, m[1].xyz, m[2].xyz);
}

// NOTE: applies "stereo adjust"
fn ws_to_spherical_coords(ws: vec3<f32>) -> vec2<f32> {
  var theta = atan2(ws.x, ws.z);
  let phi = asin(-ws.y); // y is flipped
  let angle = camera_state.stereo_adjust;
  return vec2<f32>(theta + angle, phi);
}

fn stereo_adjust_ws(ws: vec3<f32>) -> vec3<f32> {
  // TODO: technically can do sin/cos on CPU, but also, who cares... HW can handle it easily
  // NOTE: in theory good driver shader compiler can figure out that sin/cos expressions come from a uniform
  // and precalc it? can it?
  let angle = camera_state.stereo_adjust;
  let sina = sin(angle);
  let cosa = cos(angle);
  let m = mat3x3<f32>(
    cosa, 0.0, -sina,
    0.0, 1.0, 0.0,
    sina, 0.0, cosa,
  );
  return m * ws;
}

fn stereo_left_right(uv: vec2<f32>) -> vec2<f32> {
  var uv = uv;
  uv.x = uv.x / 2.0;
  if (camera_state.eye_index == 1u) {
    uv.x = uv.x + 0.5;
  }
  return uv;
}

fn stereo_top_bottom(uv: vec2<f32>) -> vec2<f32> {
  var uv = uv;
  uv.y = uv.y / 2.0;
  if (camera_state.eye_index == 1u) {
    uv.y = uv.y + 0.5;
  }
  return uv;
}

fn stereo(uv: vec2<f32>) -> vec2<f32> {
  if (camera_state.mode == 0u) {
    return uv; // mono
  } else if (camera_state.mode == 1u) {
    return stereo_left_right(uv);
  } else {
    return stereo_top_bottom(uv);
  }
}

fn equirectangular_360(sc: vec2<f32>) -> vec2<f32> {
  var uv = sc / PI;
  uv.x = (uv.x + 1.0) / 2.0;
  uv.y = uv.y + 0.5;
  return stereo(uv);
}

fn equirectangular_180(sc: vec2<f32>) -> vec2<f32> {
  var uv = sc / PI;
  uv.x = uv.x + 0.5;
  uv.y = uv.y + 0.5;
  return stereo(uv);
}

fn fisheye_180(ws: vec3<f32>) -> vec2<f32> {
  let ws = stereo_adjust_ws(ws);
  let phi = atan2(sqrt(ws.x * ws.x + ws.y * ws.y), ws.z);
  let r = 2.0 * phi / PI;
  let theta = atan2(ws.y, ws.x);
  var uv = (r * vec2<f32>(cos(theta), sin(theta)) + 1.0) / 2.0;

  // flip v
  uv.y = 1.0 - uv.y;
  return stereo(uv);
}

fn sample_cube(ws: vec3<f32>, face_index: ptr<function,u32>) -> vec2<f32> {
  let vabs = abs(ws);
  var ma = 0.0;
  var uv = vec2<f32>(0.0, 0.0);
  if (vabs.z >= vabs.x && vabs.z >= vabs.y) {
    if (ws.z < 0.0) {
      *face_index = 5u;
      uv = vec2<f32>(-ws.x, -ws.y);
    } else {
      *face_index = 4u;
      uv = vec2<f32>(ws.x, -ws.y);
    }
    ma = 0.5 / vabs.z;
  } else if (vabs.y >= vabs.x) {
    if (ws.y < 0.0) {
      *face_index = 3u;
      uv = vec2<f32>(ws.x, -ws.z);
    } else {
      *face_index = 2u;
      uv = vec2<f32>(ws.x, ws.z);
    }
    ma = 0.5 / vabs.y;
  } else {
    if (ws.x < 0.0) {
      *face_index = 1u;
      uv = vec2<f32>(ws.z, -ws.y);
    } else {
      *face_index = 0u;
      uv = vec2<f32>(-ws.z, -ws.y);
    }
    ma = 0.5 / vabs.x;
  }
  return uv * ma;
}

fn eac(ws: vec3<f32>) -> vec2<f32> {
  let ws = stereo_adjust_ws(ws);

  var face_index: u32;
  var uv = sample_cube(ws, &face_index);

  if (face_index == 2u) {
    uv = vec2<f32>(uv.y, -uv.x);
  } else if (face_index == 3u) {
    uv = vec2<f32>(uv.y, -uv.x);
  } else if (face_index == 5u) {
    uv = vec2<f32>(-uv.y, uv.x);
  }

  // that's the core difference between EAC and standard cubemap
  uv = ((2.0 / PI) * atan(2.0 * uv) + 0.5);

  // 0                   1 (U)
  // --------------------
  // |        |         |
  // | right  |   top   |
  // |        |         |
  // --------------------
  // |        |         |
  // | front  |  back   |
  // |        |         |
  // --------------------
  // |        |         |
  // | left   | bottom  |
  // |        |         |
  // --------------------
  // 1 (V)

  // 0 - right
  // 1 - left
  // 2 - top
  // 3 - bottom
  // 4 - front
  // 5 - back

  let step = vec2<f32>(1.0, 1.0) / vec2<f32>(3.0, 2.0);
  if (camera_state.mode != 0u) {
    if (face_index == 4u) {
      uv.x = 1.0 - uv.x;
    }
  }
  if (face_index == 0u) {
    uv = step * vec2<f32>(2.0, 0.0) + uv * step;
  } else if (face_index == 1u) {
    uv = step * vec2<f32>(0.0, 0.0) + uv * step;
  } else if (face_index == 2u) {
    uv = step * vec2<f32>(2.0, 1.0) + uv * step;
  } else if (face_index == 3u) {
    uv = step * vec2<f32>(0.0, 1.0) + uv * step;
  } else if (face_index == 4u) {
    uv = step * vec2<f32>(1.0, 0.0) + uv * step;
  } else if (face_index == 5u) {
    uv = step * vec2<f32>(1.0, 1.0) + uv * step;
  }
  if (camera_state.mode != 0u) {
    if (face_index != 4u) {
      uv.x = 1.0 - uv.x;
    }
    uv = vec2<f32>(uv.y, uv.x);
  }
  return stereo(uv);
}

fn weird_checkerboard(ws: vec3<f32>) -> vec4<f32> {
  if (ws.y > 0.0) {
    return vec4<f32>(ws.y, ws.y, ws.y, 1.0);
  } else {
    var xz = ws.xz / min(-0.2, ws.y);
    xz = xz * 4.0;
    var c = floor(xz.x) + floor(xz.y);
    c = fract(c * 0.5);
    c = c * 2.0;
    return vec4<f32>(c, c, c, 1.0);
  }
}
