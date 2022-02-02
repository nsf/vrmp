use glam::{Mat4, Vec3};

use crate::danger;

pub struct VRInfo {
    // recommended eye size as returned from openvr api
    pub recommended_eye_size: (u32, u32),
    // actual ipd calculated from eye matrices
    pub ipd: f32,
    pub eye_w: u32,
    pub eye_h: u32,
    pub left_eye_proj_mat: Mat4,
    pub left_eye_inv_proj_mat: Mat4,
    pub left_eye_to_head_mat: Mat4,
    pub right_eye_proj_mat: Mat4,
    pub right_eye_inv_proj_mat: Mat4,
    pub right_eye_to_head_mat: Mat4,

    pub left_eye: danger::vulkan::EyeData,
    pub right_eye: danger::vulkan::EyeData,

    pub hmd_mat: Mat4,
    pub orig_hmd_mat: Mat4,
}

impl VRInfo {
    pub fn create(vr_ctx: &libopenvr::Context, wgpu_device: &wgpu::Device) -> VRInfo {
        let recommended_eye_size = vr_ctx.system.recommended_render_target_size();
        let (eye_w, eye_h) = recommended_eye_size;
        let eye_w = eye_w * 2;
        let eye_h = eye_h * 2;
        let left_eye_proj_mat = vr_ctx.system.get_projection_matrix(libopenvr::Eye::Left, 0.1, 100.0);
        let left_eye_inv_proj_mat = left_eye_proj_mat.inverse();
        let left_eye_to_head_mat = vr_ctx.system.get_eye_to_head_transform(libopenvr::Eye::Left).inverse();
        let right_eye_proj_mat = vr_ctx.system.get_projection_matrix(libopenvr::Eye::Right, 0.1, 100.0);

        let right_eye_inv_proj_mat = right_eye_proj_mat.inverse();
        let right_eye_to_head_mat = vr_ctx.system.get_eye_to_head_transform(libopenvr::Eye::Right).inverse();

        let lpt = left_eye_to_head_mat.transform_point3(Vec3::splat(0.0));
        let rpt = right_eye_to_head_mat.transform_point3(Vec3::splat(0.0));
        let ipd = lpt.distance(rpt);

        let left_eye = danger::vulkan::EyeData::create(wgpu_device, eye_w, eye_h);
        let right_eye = danger::vulkan::EyeData::create(wgpu_device, eye_w, eye_h);
        VRInfo {
            recommended_eye_size,
            ipd,
            eye_w,
            eye_h,
            left_eye_proj_mat,
            left_eye_inv_proj_mat,
            left_eye_to_head_mat,
            right_eye_proj_mat,
            right_eye_inv_proj_mat,
            right_eye_to_head_mat,
            left_eye,
            right_eye,
            hmd_mat: Mat4::IDENTITY,
            orig_hmd_mat: Mat4::IDENTITY,
        }
    }
}
