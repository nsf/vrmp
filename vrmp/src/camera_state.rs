use bytemuck_derive::{Pod, Zeroable};
use glam::{Mat3, Mat4};

use crate::{
    enums::{Mode, Projection},
    filedb::FileData,
    imgui::general::General,
};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct CameraState {
    mvp: Mat4,
    inverse_projection: Mat4,
    view_orientation: Mat4,
    eye_index: u32,
    mode: u32,
    stereo_adjust: f32,
    shader_debug: f32,
}

impl CameraState {
    pub fn from_proj_and_view(
        proj_mat: Mat4,
        view_mat: Mat4,
        world_origin: Mat4,
        eye_index: u32,
        fdata: Option<&FileData>,
        g: &General,
    ) -> CameraState {
        let inverse_projection = proj_mat.inverse();
        let view_orientation = (view_mat * Mat4::from_mat3(Mat3::from_mat4(world_origin))).inverse();
        let stereo_adjust = fdata
            .map(|d| {
                cond!(
                    d.projection == Projection::Flat,
                    d.stereo_convergence_flat,
                    d.stereo_convergence.to_radians()
                )
            })
            .unwrap_or(0.0)
            * cond!(eye_index == 0, -1.0, 1.0);
        let mode = fdata.map(|d| d.mode).unwrap_or(Mode::Mono);
        let (eye_index, mode) = match mode {
            Mode::Mono => (eye_index, 0),
            Mode::LeftRight => (eye_index, 1),
            Mode::RightLeft => (cond!(eye_index == 0, 1, 0), 1),
            Mode::TopBottom => (eye_index, 2),
            Mode::BottomTop => (cond!(eye_index == 0, 1, 0), 2),
        };
        CameraState {
            mvp: proj_mat * view_mat,
            inverse_projection,
            view_orientation,
            eye_index,
            mode,
            stereo_adjust: stereo_adjust,
            shader_debug: g.shader_debug,
        }
    }
}
