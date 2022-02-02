use crate::{
    config::Config,
    enums::AspectRatio,
    pipeline::{fullscreen_triangle::FullscreenTriangle, textured_quad::TexturedQuad},
};
use glam::{Mat4, Vec3};

#[derive(Copy, Clone)]
pub enum VideoRenderer<'a> {
    FTri(&'a FullscreenTriangle),
    TQuad(&'a TexturedQuad, Mat4),
}

pub struct Scene<'a> {
    pub queue: &'a wgpu::Queue,
    pub device: &'a wgpu::Device,
    pub color: &'a wgpu::TextureView,
    pub depth: &'a wgpu::TextureView,
    pub video: VideoRenderer<'a>,
    pub lines_pipeline: &'a wgpu::RenderPipeline,
    pub lines_buf: &'a wgpu::Buffer,
    pub camera_bgrp: &'a wgpu::BindGroup,
    pub video_bgrp: &'a wgpu::BindGroup,
    pub tquad_imgui: &'a TexturedQuad,
    pub vscreen: Option<&'a crate::vscreen::VScreen>,
    pub config: &'a Config,
    pub debug_matrices: &'a [Mat4],
    pub world_origin: Mat4,
    pub ui_origin: Mat4,
}

pub fn render_scene(s: &Scene) {
    let mut encoder = s
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: s.color,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            }],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: s.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });
        rpass.set_bind_group(0, s.camera_bgrp, &[]);

        // video
        rpass.set_bind_group(1, s.video_bgrp, &[]);
        match s.video {
            VideoRenderer::FTri(tri) => {
                rpass.set_pipeline(&tri.pipeline);
                rpass.draw(0..3, 0..1);
            }
            VideoRenderer::TQuad(tquad, m) => {
                let m = s.world_origin * m;
                rpass.set_pipeline(&tquad.pipeline);
                rpass.set_vertex_buffer(0, tquad.vertex_buf.slice(..));
                rpass.set_push_constants(wgpu::ShaderStages::VERTEX, 0, bytemuck::bytes_of(&m));
                rpass.draw(0..6, 0..1);
            }
        }

        // rpass.set_pipeline(s.lines_pipeline);
        // rpass.set_vertex_buffer(0, s.lines_buf.slice(..));
        // for m in s.debug_matrices {
        //     rpass.set_push_constants(wgpu::ShaderStages::VERTEX, 0, bytemuck::bytes_of(m));
        //     rpass.draw(0..6, 0..1);
        // }

        if let Some(vscreen) = s.vscreen {
            let ui_angle = s.config.ui_angle;
            let ui_distance = s.config.ui_distance;
            let ui_scale = s.config.ui_scale;

            let rot_mat = Mat4::from_rotation_x(ui_angle.to_radians());
            let tr_mat = Mat4::from_translation(Vec3::new(0.0, 0.0, ui_distance));
            let scale_mat = TexturedQuad::scale_for_wh(vscreen.width, vscreen.height, ui_scale, AspectRatio::One);

            rpass.set_pipeline(&s.tquad_imgui.pipeline);
            rpass.set_bind_group(1, &vscreen.bind_group, &[]);
            let pos = s.ui_origin * rot_mat * tr_mat * scale_mat;
            rpass.set_push_constants(wgpu::ShaderStages::VERTEX, 0, bytemuck::bytes_of(&pos));
            rpass.set_vertex_buffer(0, s.tquad_imgui.vertex_buf.slice(..));
            rpass.draw(0..6, 0..1);
        }
    }
    s.queue.submit(Some(encoder.finish()));
}
