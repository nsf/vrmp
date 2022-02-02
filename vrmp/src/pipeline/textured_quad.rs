use std::{borrow::Cow, mem};

use bytemuck_derive::{Pod, Zeroable};
use glam::{Mat4, Vec2, Vec3};

use crate::enums::AspectRatio;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Vertex {
    position: glam::Vec3,
    texcoord: glam::Vec2,
}

pub struct TexturedQuad {
    pub pipeline: wgpu::RenderPipeline,
    pub vertex_buf: wgpu::Buffer,
}

impl TexturedQuad {
    pub fn create(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_target_state: wgpu::ColorTargetState,
        pipeline_layout: &wgpu::PipelineLayout,
        shader_source: &str,
    ) -> TexturedQuad {
        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(shader_source)),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[color_target_state],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Cw,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let vertex_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (mem::size_of::<Vertex>() * 6) as _,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        queue.write_buffer(
            &vertex_buf,
            0,
            bytemuck::cast_slice(&[
                Vertex {
                    position: Vec3::new(-0.5, -0.5, 0.0),
                    texcoord: Vec2::new(0.0, 0.0),
                },
                Vertex {
                    position: Vec3::new(0.5, -0.5, 0.0),
                    texcoord: Vec2::new(1.0, 0.0),
                },
                Vertex {
                    position: Vec3::new(-0.5, 0.5, 0.0),
                    texcoord: Vec2::new(0.0, 1.0),
                },
                Vertex {
                    position: Vec3::new(-0.5, 0.5, 0.0),
                    texcoord: Vec2::new(0.0, 1.0),
                },
                Vertex {
                    position: Vec3::new(0.5, -0.5, 0.0),
                    texcoord: Vec2::new(1.0, 0.0),
                },
                Vertex {
                    position: Vec3::new(0.5, 0.5, 0.0),
                    texcoord: Vec2::new(1.0, 1.0),
                },
            ]),
        );

        TexturedQuad { pipeline, vertex_buf }
    }

    // create a scale matrix based on w/h (aspect ratio)
    // we keep height at 1 then calculate width based on aspect ratio and apply scale, thus scale is the
    // meters size height-wise
    pub fn scale_for_wh(w: u32, h: u32, scale: f32, ar: AspectRatio) -> Mat4 {
        let mut aspect_ratio = w as f32 / h as f32;
        match ar {
            AspectRatio::Half => aspect_ratio *= 0.5,
            AspectRatio::One => aspect_ratio *= 1.0,
            AspectRatio::Two => aspect_ratio *= 2.0,
        }
        let sy = 1.0f32;
        let sx = sy * aspect_ratio;
        Mat4::from_scale(Vec3::new(sx * scale, sy * scale, 1.0))
    }
}
