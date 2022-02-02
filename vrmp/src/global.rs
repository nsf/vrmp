use std::{
    borrow::Cow,
    mem,
    sync::Arc,
    time::{Duration, Instant},
};

use argh::FromArgs;
use bytemuck_derive::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec2, Vec3};
use sdl2::{
    event::{Event, WindowEvent},
    keyboard::Keycode,
    mouse::MouseButton,
};
use wgpu_hal::InstanceFlags;

use crate::{
    action::{Action, ActionBin},
    camera_state::CameraState,
    config::{Config, ConfigSyncer},
    danger::{self, egl_bridge::EGLContext},
    enums::{AspectRatio, Projection},
    filedb::FileDB,
    imgui::font_awesome,
    imgui::{file_browser::ImguiFileBrowser, general::General},
    pipeline::{fullscreen_triangle::FullscreenTriangle, textured_quad::TexturedQuad},
    scene::{render_scene, Scene, VideoRenderer},
    vrinfo::VRInfo,
    vscreen::VScreen,
};
use crate::{filedb::load_file_size_and_hash, tracks::Tracks};

fn reset_origin(cam_mat: Mat4) -> Mat4 {
    let (_, rot, tr) = cam_mat.inverse().to_scale_rotation_translation();
    let y = rot.to_euler(glam::EulerRot::YXZ).0;
    return Mat4::from_translation(tr) * Mat4::from_rotation_y(y);
}

fn create_depth_texture(device: &wgpu::Device, w: u32, h: u32) -> wgpu::TextureView {
    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: w,
            height: h,
            ..Default::default()
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
    });
    depth_texture.create_view(&wgpu::TextureViewDescriptor::default())
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct LineVertex {
    position: glam::Vec3,
    color: glam::Vec3,
}

#[derive(FromArgs)]
/// VR media player
pub struct Arguments {
    #[argh(switch)]
    /// enable VR mode (will initialize and use openvr library)
    pub vr: bool,

    #[argh(switch)]
    /// enable vulkan debug and validation layers
    pub validation_layers: bool,
}

pub struct Global {
    // rust assumes there are no dependencies between struct members and it runs destructors in declaration order
    // (but my code is unsafe here and there are dependencies between struct members, I should probably do something
    // about it)
    //                   ##
    //                   ##
    //                   ##
    //                   ##
    //               ##  ##  ##
    //                 ######
    //                   ##

    // various stuff that is trivially destructable, no need to order it
    time: Instant,
    delta: Duration,
    delta_accum_secs: f32,
    delta_accum_fast: f32,
    runtime_secs: u64,
    is_per_sec_update: bool,
    is_fast_update: bool,
    camera_state: CameraState,
    swap_z: Mat4,
    ui_origin: Mat4,
    world_origin: Mat4,
    proj_mat: Mat4,
    view_mat: Mat4,
    move_forward: bool,
    move_backward: bool,
    move_right: bool,
    move_left: bool,
    cam_pos: Vec3,
    cam_quat: Quat,
    is_running: bool,
    is_gui: bool,
    suboptimal: bool,
    surface_config: wgpu::SurfaceConfiguration,
    config_syncer: ConfigSyncer,
    async_size: (Option<u32>, Option<u32>),
    current_file_path: Option<String>,
    current_file_duration: Option<u32>,
    current_file_key: Option<(u64, u64)>,
    current_file_tracks: Option<Tracks>,
    filedb: FileDB,
    action_bin: ActionBin,

    // wgpu resources, generally it's safe to destroy them in arbitrary order
    vr_info: Option<VRInfo>,
    vscreen: VScreen,
    tquad_shared_tex: TexturedQuad,
    tquad_imgui: TexturedQuad,

    ftri_equirectangular_360: FullscreenTriangle,
    ftri_equirectangular_180: FullscreenTriangle,
    ftri_fisheye_180: FullscreenTriangle,
    ftri_equiangular_cubemap: FullscreenTriangle,

    camera_state_uniform_buf: wgpu::Buffer,
    lines_buf: wgpu::Buffer,
    camera_bgrp: wgpu::BindGroup,
    lines_pipeline: wgpu::RenderPipeline,
    depth_view: wgpu::TextureView,
    black_texture_bgrp: wgpu::BindGroup,

    // I destroy these manually in shutdown function, at least their unsafe part
    shared_tex: danger::shared_texture::SharedTexture,
    gpu: danger::vulkan::VulkanWGPU,

    // mpv
    mpv_render: Box<libmpv::RenderContext>,
    mpv: Box<libmpv::Context>,

    // imgui
    imgui_general: General,
    imgui_file_browser: ImguiFileBrowser,
    imgui_renderer: imgui_wgpu::Renderer,
    imgui: imgui::Context,

    // sdl2
    sdl_event_pump: sdl2::EventPump,
    sdl_window: sdl2::video::Window,
    sdl_video_subsystem: sdl2::VideoSubsystem,
    sdl_context: sdl2::Sdl,

    // vr
    vr: Option<Box<libopenvr::Context>>, // destroyed manually

    // egl
    egl: Box<EGLContext>,
}

impl Global {
    pub fn init() -> Global {
        let config_syncer = ConfigSyncer::new(Config::load().expect("failed loading config"));
        let args: Arguments = argh::from_env();
        let egl = danger::egl_bridge::load_egl();
        log::info!("loading app");

        let vr = args
            .vr
            .then(|| libopenvr::Context::create(libopenvr::ApplicationType::Scene));

        sdl2::hint::set("SDL_VIDEO_X11_FORCE_EGL", "1");

        let sdl_context = sdl2::init().unwrap();
        let sdl_video_subsystem = sdl_context.video().unwrap();

        let sdl_window = sdl_video_subsystem
            .window("vrmp", 1920, 1080)
            .resizable()
            .build()
            .unwrap();

        sdl_context.mouse().show_cursor(false);
        sdl_context.mouse().set_relative_mouse_mode(true);

        egl.egl
            .make_current(egl.display, Some(egl.surface), Some(egl.surface), Some(egl.context))
            .unwrap();

        let mpv = libmpv::Context::create();
        mpv.initialize();

        mpv.observe_property("sid");
        mpv.observe_property("vid");
        mpv.observe_property("aid");
        mpv.observe_property("pause");
        mpv.observe_property("hwdec");
        mpv.observe_property("hwdec-current");

        // NOTE: mpv uses references to egl here in its event callbacks, please make sure it's kept in a Box<_>,
        // otherwise pointer will be invalidated after move out of init() function we're in
        let mpv_render = unsafe { mpv.create_render_context(&egl.egl, &sdl_window) };

        let (w, h) = sdl_window.drawable_size();
        let gpu = unsafe {
            danger::vulkan::VulkanWGPU::create(&danger::vulkan::LoadVulkanWGPUParams {
                vr_ctx: vr.as_ref().map(|v| v.as_ref()),
                window: &sdl_window,
                features: wgpu::Features::default() | wgpu::Features::PUSH_CONSTANTS,
                limits: wgpu::Limits {
                    max_push_constant_size: 4 * 4 * 4, // I want to push mat4x4
                    ..Default::default()
                },
                flags: cond!(args.validation_layers, InstanceFlags::all(), InstanceFlags::empty()),
            })
        };

        let shared_texture_bind_group_layout =
            Arc::new(gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                }],
            }));

        let mut imgui = imgui::Context::create();
        imgui.set_ini_filename(None);
        // imgui.io_mut().font_global_scale = 2.0;
        imgui.io_mut().mouse_draw_cursor = true;
        imgui.fonts().add_font(&[
            imgui::FontSource::TtfData {
                data: include_bytes!("Roboto-Regular.ttf"),
                size_pixels: 26.0,
                config: Some(imgui::FontConfig {
                    glyph_ranges: imgui::FontGlyphRanges::cyrillic(),
                    ..Default::default()
                }),
            },
            imgui::FontSource::TtfData {
                data: include_bytes!("fa-solid-900.ttf"),
                size_pixels: 26.0,
                config: Some(imgui::FontConfig {
                    glyph_ranges: imgui::FontGlyphRanges::from_slice(&[font_awesome::MIN, font_awesome::MAX, 0]),
                    ..Default::default()
                }),
            },
        ]);

        let mut vscreen = crate::vscreen::VScreen::create(&gpu.device, &shared_texture_bind_group_layout, 2560, 1440);
        vscreen.imgui_init(&mut imgui);

        let renderer_config = imgui_wgpu::RendererConfig {
            texture_format: wgpu::TextureFormat::Bgra8Unorm,
            ..Default::default()
        };
        let imgui_renderer = imgui_wgpu::Renderer::new(&mut imgui, &gpu.device, &gpu.queue, renderer_config);

        let shared_tex = danger::shared_texture::SharedTexture::create(
            &gpu.ash_instance,
            &gpu.ash_device,
            gpu.vk_physical_device,
            &gpu.device,
            shared_texture_bind_group_layout.clone(),
            512,
            512,
        );

        // Load the shaders from disk
        let lines_shader = gpu.device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_shader!("lines.wgsl"))),
        });

        let bind_group_layout = gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(mem::size_of::<CameraState>() as _),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout, &shared_texture_bind_group_layout],
            push_constant_ranges: &[wgpu::PushConstantRange {
                range: 0..64,
                stages: wgpu::ShaderStages::VERTEX,
            }],
        });

        let swapchain_format = gpu.surface.get_preferred_format(&gpu.adapter).unwrap();

        let vr_info = vr.as_ref().map(|vr_ctx| VRInfo::create(&vr_ctx, &gpu.device));
        if let Some(vr_info) = &vr_info {
            log::info!(
                "Recommended Eye Resolution: {}x{}",
                vr_info.recommended_eye_size.0,
                vr_info.recommended_eye_size.1
            );
            log::info!("Eye Resolution: {}x{}", vr_info.eye_w, vr_info.eye_h);
            log::info!("IPD: {}", vr_info.ipd);
        }

        let lines_pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &lines_shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: mem::size_of::<LineVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &lines_shader,
                entry_point: "fs_main",
                targets: &[swapchain_format.into()],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
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

        let depth_view = create_depth_texture(&gpu.device, w, h);

        //---------------------------------------------------------------------------------

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: w,
            height: h,
            present_mode: wgpu::PresentMode::Mailbox,
        };

        gpu.surface.configure(&gpu.device, &surface_config);

        //---------------------------------------------------------------------------------
        let lines_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (mem::size_of::<LineVertex>() * 6) as _,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        gpu.queue.write_buffer(
            &lines_buf,
            0,
            bytemuck::cast_slice(&[
                LineVertex {
                    position: Vec3::new(0.0, 0.0, 0.0),
                    color: Vec3::new(1.0, 0.0, 0.0),
                },
                LineVertex {
                    position: Vec3::new(1.0, 0.0, 0.0),
                    color: Vec3::new(1.0, 0.0, 0.0),
                },
                LineVertex {
                    position: Vec3::new(0.0, 0.0, 0.0),
                    color: Vec3::new(0.0, 1.0, 0.0),
                },
                LineVertex {
                    position: Vec3::new(0.0, 1.0, 0.0),
                    color: Vec3::new(0.0, 1.0, 0.0),
                },
                LineVertex {
                    position: Vec3::new(0.0, 0.0, 0.0),
                    color: Vec3::new(0.0, 0.0, 1.0),
                },
                LineVertex {
                    position: Vec3::new(0.0, 0.0, 1.0),
                    color: Vec3::new(0.0, 0.0, 1.0),
                },
            ]),
        );

        let camera_state_uniform_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: mem::size_of::<CameraState>() as _,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let linear_sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let camera_bgrp = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_state_uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&linear_sampler),
                },
            ],
            label: None,
        });

        let black_texture_bgrp = {
            let texels: &[u8] = &[0, 0, 0, 255];
            let texture_extent = wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            };
            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: texture_extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            });
            let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            gpu.queue.write_texture(
                texture.as_image_copy(),
                &texels,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(std::num::NonZeroU32::new(4).unwrap()),
                    rows_per_image: None,
                },
                texture_extent,
            );

            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &shared_texture_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                }],
            })
        };

        let tquad_shared_tex = TexturedQuad::create(
            &gpu.device,
            &gpu.queue,
            swapchain_format.into(),
            &pipeline_layout,
            include_shader!("proj_flat.wgsl"),
        );
        let tquad_imgui = TexturedQuad::create(
            &gpu.device,
            &gpu.queue,
            wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            },
            &pipeline_layout,
            include_shader!("textured_quad.wgsl"),
        );

        let ftri_equirectangular_360 = FullscreenTriangle::create(
            &gpu.device,
            swapchain_format.into(),
            &pipeline_layout,
            include_shader!("proj_equirectangular_360.wgsl"),
        );
        let ftri_equirectangular_180 = FullscreenTriangle::create(
            &gpu.device,
            swapchain_format.into(),
            &pipeline_layout,
            include_shader!("proj_equirectangular_180.wgsl"),
        );
        let ftri_fisheye_180 = FullscreenTriangle::create(
            &gpu.device,
            swapchain_format.into(),
            &pipeline_layout,
            include_shader!("proj_fisheye_180.wgsl"),
        );
        let ftri_cubemap = FullscreenTriangle::create(
            &gpu.device,
            swapchain_format.into(),
            &pipeline_layout,
            include_shader!("proj_equiangular_cubemap.wgsl"),
        );
        //---------------------------------------------------------------------------------

        let mut filedb = FileDB::load();
        let imgui_file_browser = ImguiFileBrowser::new(&mut filedb);
        let imgui_general = General::new();
        let cam_quat = Quat::IDENTITY;
        let cam_pos = Vec3::new(0.0, 0.0, 0.0);
        let proj_mat = Mat4::perspective_lh(90f32.to_radians(), w as f32 / h as f32, 0.01, 100.0);
        let view_mat = Mat4::from_rotation_translation(cam_quat.inverse(), -cam_pos);
        let camera_state = CameraState::from_proj_and_view(proj_mat, view_mat, Mat4::IDENTITY, 0, None, &imgui_general);
        let time = Instant::now();
        let sdl_event_pump = sdl_context.event_pump().unwrap();
        let swap_z = Mat4::from_scale(Vec3::new(1.0, 1.0, -1.0));

        Global {
            action_bin: ActionBin::create(),
            suboptimal: false,
            filedb,
            async_size: (None, None),
            current_file_path: None,
            current_file_duration: None,
            current_file_key: None,
            current_file_tracks: None,
            config_syncer,
            egl,
            vr,
            sdl_context,
            sdl_video_subsystem,
            sdl_window,
            sdl_event_pump,
            imgui,
            imgui_renderer,
            imgui_file_browser,
            imgui_general,
            mpv,
            mpv_render,
            gpu,
            shared_tex,
            depth_view,
            lines_pipeline,
            black_texture_bgrp,
            camera_bgrp,
            lines_buf,
            camera_state_uniform_buf,
            vscreen,
            tquad_shared_tex,
            tquad_imgui,
            ftri_equirectangular_360,
            ftri_equirectangular_180,
            ftri_fisheye_180,
            ftri_equiangular_cubemap: ftri_cubemap,
            vr_info,
            camera_state,
            swap_z,
            ui_origin: Mat4::IDENTITY,
            world_origin: Mat4::IDENTITY,
            proj_mat,
            view_mat,
            delta_accum_secs: 0.0,
            delta_accum_fast: 0.0,
            runtime_secs: 0,
            move_forward: false,
            move_backward: false,
            move_right: false,
            move_left: false,
            cam_pos,
            cam_quat,
            surface_config,
            is_per_sec_update: false,
            is_fast_update: false,
            delta: Default::default(),
            time,
            is_running: true,
            is_gui: false,
        }
    }

    fn current_camera_mat(&self) -> Mat4 {
        if let Some(vr_info) = &self.vr_info {
            vr_info.hmd_mat
        } else {
            Mat4::from_quat(self.cam_quat.inverse()) * Mat4::from_translation(-self.cam_pos)
        }
    }

    pub fn main_loop(&mut self) {
        self.update_delta();
        self.update_mpv();
        self.update_imgui();
        if self.is_per_sec_update {
            self.per_second_update();
        }
        if self.is_fast_update {
            self.fast_update();
        }

        self.before_vk_render();

        let frame = self.vk_render();

        self.after_vk_render();
        self.vr_present();
        self.vk_present(frame);
        self.gl_render();

        self.handle_sdl2_events();
        self.handle_action_bin();

        if self.suboptimal {
            self.suboptimal = false;
            self.gpu.surface.configure(&self.gpu.device, &self.surface_config);
        }

        self.wait_get_hmd_pose();
        self.is_per_sec_update = false;
        self.is_fast_update = false;
    }

    pub fn update_delta(&mut self) {
        let now = Instant::now();
        self.delta = now - self.time;
        self.time = now;

        self.delta_accum_secs += self.delta.as_secs_f32();
        if self.delta_accum_secs > 1.0 {
            self.delta_accum_secs -= 1.0;
            self.runtime_secs += 1;
            self.is_per_sec_update = true;
        }
        self.delta_accum_fast += self.delta.as_secs_f32();
        if self.delta_accum_fast > 0.25 {
            self.delta_accum_fast -= 0.25;
            self.is_fast_update = true;
        }
    }

    pub fn update_imgui(&mut self) {
        self.vscreen.imgui_prepare_frame(&mut self.imgui);
    }

    fn reset_current_file(&mut self) {
        self.current_file_path = None;
        self.current_file_duration = None;
        self.current_file_tracks = None;
    }

    pub fn update_mpv(&mut self) {
        for ev in self.mpv.drain_events() {
            match ev {
                libmpv::Event::VideoReconfig => {
                    self.async_size = (None, None);
                    self.mpv.get_size_async();
                }
                libmpv::Event::EndFile => {
                    self.reset_current_file();
                }
                libmpv::Event::FileLoaded => {
                    self.reset_current_file();
                    self.mpv.get_path_async();
                    self.mpv.get_video_params_async();
                    self.mpv.get_track_list_async();
                }
                libmpv::Event::PropertyChange(name) => match name.as_str() {
                    "pause" => self.mpv.get_pause_async(),
                    "aid" => self.mpv.get_aid_async(),
                    "vid" => self.mpv.get_vid_async(),
                    "sid" => self.mpv.get_sid_async(),
                    "hwdec" => self.mpv.get_hwdec_async(),
                    "hwdec-current" => self.mpv.get_hwdec_current_async(),
                    _ => {}
                },
                libmpv::Event::Property(p) => match (p.name.as_ref(), p.value) {
                    ("hwdec-current", libmpv::PropertyValue::String(v)) => self.imgui_general.hwdec_current = v,
                    ("hwdec", libmpv::PropertyValue::String(v)) => self.imgui_general.hwdec = v,
                    ("path", libmpv::PropertyValue::String(v)) => self.on_mpv_file_loaded(v),
                    ("width", libmpv::PropertyValue::I64(v)) => self.async_size.0 = Some(v as u32),
                    ("height", libmpv::PropertyValue::I64(v)) => self.async_size.1 = Some(v as u32),
                    ("duration", libmpv::PropertyValue::I64(v)) => self.on_mpv_duration_changed(v as u32),
                    ("percent-pos", libmpv::PropertyValue::F64(v)) => self.on_mpv_percent_pos_change(v),
                    ("vid", libmpv::PropertyValue::I64(v)) => {
                        if let Some(t) = &mut self.current_file_tracks {
                            t.vid = v;
                        }
                    }
                    ("aid", libmpv::PropertyValue::I64(v)) => {
                        if let Some(t) = &mut self.current_file_tracks {
                            t.aid = v;
                        }
                    }
                    ("sid", libmpv::PropertyValue::I64(v)) => {
                        if let Some(t) = &mut self.current_file_tracks {
                            t.sid = v;
                        }
                    }
                    ("track-list", libmpv::PropertyValue::Node(n)) => {
                        self.current_file_tracks = Some(Tracks::parse(&n));
                        self.mpv.get_vid_async();
                        self.mpv.get_aid_async();
                        self.mpv.get_sid_async();
                    }
                    ("pause", libmpv::PropertyValue::Bool(v)) => self.imgui_general.playing = !v, // this one is purely visual
                    _ => {}
                },
            }
        }
        if let (Some(w), Some(h)) = self.async_size {
            self.shared_tex.request_resize(w, h);
            self.async_size = (None, None);
        }
        self.mpv_render.update_maybe();
    }

    pub fn on_mpv_file_loaded(&mut self, v: String) {
        if let Some(key) = load_file_size_and_hash(&v) {
            if let Err(e) = self.filedb.preload_file(key.0, key.1) {
                log::error!("failed preloading file: {}", e);
            }
            self.current_file_key = Some(key);
        }
        self.current_file_path = Some(v);
    }

    pub fn on_mpv_duration_changed(&mut self, v: u32) {
        self.imgui_general.duration = v;
        self.current_file_duration = Some(v);
    }

    pub fn on_mpv_percent_pos_change(&mut self, v: f64) {
        self.imgui_general.percent_pos = v;
        if let Some(key) = self.current_file_key {
            let e = self.filedb.get_file_mut(key);
            e.mark_as_seen(v);
        }
    }

    pub fn per_second_update(&mut self) {
        // I'm not sure if duration is available right after "FILE_LOADED", I should probably experiment with this
        self.mpv.get_duration_async();

        self.config_syncer.save_maybe();
        self.filedb.save_to_disk_maybe();
    }

    pub fn fast_update(&mut self) {
        self.mpv.get_percent_pos_async();
    }

    pub fn before_vk_render(&mut self) {
        self.shared_tex.resize_maybe(
            &self.gpu.ash_instance,
            &self.gpu.ash_device,
            self.gpu.vk_physical_device,
            &self.gpu.device,
        );
        self.shared_tex.before_vk(&self.gpu.ash_device, self.gpu.vk_queue);
    }

    pub fn vk_render(&mut self) -> wgpu::SurfaceTexture {
        let fdata = self.current_file_key.and_then(|k| self.filedb.get_file(k));
        let frame = self.gpu.surface.get_current_texture().unwrap();
        self.suboptimal = frame.suboptimal;
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let projection = fdata.map(|d| d.projection).unwrap_or(Projection::Flat);
        let aspect_ratio = fdata.map(|d| d.aspect_ratio).unwrap_or(AspectRatio::One);
        let flat_distnace = fdata.map(|d| d.flat_distance).unwrap_or(3.0);
        let flat_scale = fdata.map(|d| d.flat_scale).unwrap_or(3.0);
        let scene = Scene {
            queue: &self.gpu.queue,
            device: &self.gpu.device,
            color: &view,
            depth: &self.depth_view,
            camera_bgrp: &self.camera_bgrp,
            video_bgrp: cond!(
                self.shared_tex.is_ready(),
                &self.shared_tex.vk.bind_group,
                &self.black_texture_bgrp
            ),
            video: match projection {
                Projection::Er180 => VideoRenderer::FTri(&self.ftri_equirectangular_180),
                Projection::Fisheye => VideoRenderer::FTri(&self.ftri_fisheye_180),
                Projection::Eac => VideoRenderer::FTri(&self.ftri_equiangular_cubemap),
                Projection::Er360 => VideoRenderer::FTri(&self.ftri_equirectangular_360),
                Projection::Flat => VideoRenderer::TQuad(
                    &self.tquad_shared_tex,
                    Mat4::from_translation(Vec3::new(0.0, 0.0, flat_distnace))
                        * TexturedQuad::scale_for_wh(
                            self.shared_tex.vk.width,
                            self.shared_tex.vk.height,
                            flat_scale,
                            aspect_ratio,
                        ),
                ),
            },
            lines_pipeline: &self.lines_pipeline,
            lines_buf: &self.lines_buf,
            tquad_imgui: &self.tquad_imgui,
            vscreen: cond!(self.is_gui, Some(&self.vscreen), None),
            config: self.config_syncer.get(),
            world_origin: self.world_origin,
            debug_matrices: &[
                self.world_origin,
                Mat4::IDENTITY,
                self.vr_info
                    .as_ref()
                    .map(|i| i.hmd_mat.inverse())
                    .unwrap_or(Mat4::IDENTITY),
            ],
            ui_origin: self.ui_origin,
        };

        // left eye
        if let Some(vr_info) = &self.vr_info {
            self.camera_state = CameraState::from_proj_and_view(
                vr_info.left_eye_proj_mat,
                vr_info.left_eye_to_head_mat * self.swap_z * vr_info.hmd_mat,
                self.world_origin,
                0,
                fdata,
                &self.imgui_general,
            );
            self.gpu.queue.write_buffer(
                &self.camera_state_uniform_buf,
                0,
                bytemuck::bytes_of(&self.camera_state),
            );

            render_scene(&Scene {
                color: &vr_info.left_eye.texture_view,
                depth: &vr_info.left_eye.depth_texture_view,
                ..scene
            });
        }

        // right eye
        if let Some(vr_info) = &self.vr_info {
            self.camera_state = CameraState::from_proj_and_view(
                vr_info.right_eye_proj_mat,
                vr_info.right_eye_to_head_mat * self.swap_z * vr_info.hmd_mat,
                self.world_origin,
                1,
                fdata,
                &self.imgui_general,
            );
            self.gpu.queue.write_buffer(
                &self.camera_state_uniform_buf,
                0,
                bytemuck::bytes_of(&self.camera_state),
            );

            render_scene(&Scene {
                color: &vr_info.right_eye.texture_view,
                depth: &vr_info.right_eye.depth_texture_view,
                ..scene
            });
        }

        // companion window
        self.camera_state = CameraState::from_proj_and_view(
            self.proj_mat,
            self.view_mat,
            self.world_origin,
            0,
            fdata,
            &self.imgui_general,
        );
        self.gpu.queue.write_buffer(
            &self.camera_state_uniform_buf,
            0,
            bytemuck::bytes_of(&self.camera_state),
        );

        render_scene(&Scene {
            world_origin: self.world_origin,
            ..scene
        });

        if self.is_gui {
            let imgui = &mut self.imgui;
            let ui = imgui.frame();
            let gap = 20.0;
            let [w, h] = ui.io().display_size;
            let hw = (w - (3.0 * gap)) / 2.0;
            let x0 = gap;
            let x1 = gap + hw + gap;
            self.imgui_file_browser.render(
                &mut self.action_bin,
                &mut self.config_syncer,
                &mut self.filedb,
                &ui,
                [x0, gap],
                [hw, h - 2.0 * gap],
            );
            {
                let fdata = self.current_file_key.map(|k| self.filedb.get_file_mut(k));
                self.imgui_general.render(
                    &mut self.action_bin,
                    &mut self.config_syncer,
                    self.current_file_tracks.as_ref(),
                    fdata,
                    &ui,
                    [x1, gap],
                    [hw, h - 2.0 * gap],
                );
            }

            let mut encoder: wgpu::CommandEncoder = self
                .gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

            {
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[wgpu::RenderPassColorAttachment {
                        view: &self.vscreen.texture_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
                                a: 0.1,
                            }),
                            store: true,
                        },
                    }],
                    depth_stencil_attachment: None,
                });

                self.imgui_renderer
                    .render(ui.render(), &self.gpu.queue, &self.gpu.device, &mut rpass)
                    .expect("imgui rendering failed");
            }

            self.gpu.queue.submit(Some(encoder.finish()));
        }
        frame
    }

    pub fn after_vk_render(&mut self) {
        self.shared_tex.after_vk(&self.gpu.ash_device, self.gpu.vk_queue);
    }

    pub fn vr_present(&mut self) {
        if let (Some(vr_ctx), Some(vr_info)) = (&self.vr, &self.vr_info) {
            unsafe {
                self.gpu
                    .submit_eye_textures(&vr_ctx, &vr_info.left_eye, &vr_info.right_eye);
            }
        }
    }

    pub fn vk_present(&mut self, frame: wgpu::SurfaceTexture) {
        unsafe { self.gpu.cmd_pool.submit_frame(&self.gpu.ash_device, self.gpu.vk_queue) };
        frame.present();
    }

    pub fn gl_render(&mut self) {
        {
            self.egl
                .egl
                .make_current(
                    self.egl.display,
                    Some(self.egl.surface),
                    Some(self.egl.surface),
                    Some(self.egl.context),
                )
                .unwrap();

            let fbo = self.shared_tex.gl.gl_fbo as i32;
            let width = self.shared_tex.vk.width as i32;
            let height = self.shared_tex.vk.height as i32;
            let fmt = gl::SRGB8 as i32;
            self.shared_tex
                .draw_gl(|| self.mpv_render.render_maybe(fbo, width, height, fmt));
        }
    }

    pub fn handle_sdl2_events(&mut self) {
        let mut xrel_accum = 0i32;
        let mut yrel_accum = 0i32;
        for event in self.sdl_event_pump.poll_iter() {
            // some events we always handle
            if let Event::Quit { .. }
            | Event::KeyDown {
                keycode: Some(Keycode::Escape),
                ..
            } = event
            {
                self.action_bin.put(Action::Quit);
            } else if let Event::Window {
                win_event: WindowEvent::Resized(w, h),
                ..
            } = event
            {
                assert!(w > 0 && h > 0);
                let w = w as u32;
                let h = h as u32;
                self.surface_config.width = w;
                self.surface_config.height = h;
                self.gpu.surface.configure(&self.gpu.device, &self.surface_config);
                self.depth_view = create_depth_texture(&self.gpu.device, w, h);
                self.proj_mat = Mat4::perspective_lh(90f32.to_radians(), w as f32 / h as f32, 0.01, 100.0);
            } else if let Event::MouseButtonDown {
                mouse_btn: MouseButton::Right,
                ..
            } = event
            {
                self.action_bin.put(Action::ToggleUI);
            }
            if let Event::KeyDown {
                keycode: Some(Keycode::Space),
                ..
            } = event
            {
                self.action_bin.put(Action::ResetWorldOrigin);
            }

            if self.is_gui {
                // gui only events
                self.vscreen
                    .imgui_handle_event(&mut self.imgui, &event, &self.config_syncer.get());
            } else {
                // non-gui only events
                match event {
                    Event::MouseMotion { xrel, yrel, .. } => {
                        xrel_accum += xrel;
                        yrel_accum += yrel;
                    }
                    _ => {}
                }
            }

            if !self.imgui.io().want_capture_keyboard {
                // it's ok to handle keyboard events if imgui doesn't need keyboard input
                match event {
                    Event::KeyDown { keycode, .. } => match keycode {
                        Some(Keycode::W) => self.move_forward = true,
                        Some(Keycode::S) => self.move_backward = true,
                        Some(Keycode::A) => self.move_left = true,
                        Some(Keycode::D) => self.move_right = true,
                        _ => {}
                    },
                    Event::KeyUp { keycode, .. } => match keycode {
                        Some(Keycode::W) => self.move_forward = false,
                        Some(Keycode::S) => self.move_backward = false,
                        Some(Keycode::A) => self.move_left = false,
                        Some(Keycode::D) => self.move_right = false,
                        _ => {}
                    },
                    _ => {}
                }
            }
        }

        // UPDATE COMPANION WINDOW CAMERA
        {
            if self.move_forward | self.move_backward | self.move_left | self.move_right {
                let mut motion = Vec2::new(0.0, 0.0);
                if self.move_forward {
                    motion.y += 1.0;
                }
                if self.move_backward {
                    motion.y -= 1.0;
                }
                if self.move_right {
                    motion.x += 1.0;
                }
                if self.move_left {
                    motion.x -= 1.0;
                }
                motion = motion.normalize();
                let cam_mat = Mat4::from_quat(self.cam_quat);
                let forward_vec = cam_mat.z_axis.truncate();
                let right_vec = cam_mat.x_axis.truncate();

                let speed = self.config_syncer.get().camera_movement_speed;
                self.cam_pos += forward_vec * Vec3::splat(self.delta.as_secs_f32() * speed) * motion.y;
                self.cam_pos += right_vec * Vec3::splat(self.delta.as_secs_f32() * speed) * motion.x;
            }
            if xrel_accum != 0 || yrel_accum != 0 {
                let sens = self.config_syncer.get().camera_sensitivity;
                let vrot = Quat::from_rotation_x((yrel_accum as f32 * sens).to_radians());
                let hrot = Quat::from_rotation_y((xrel_accum as f32 * sens).to_radians());
                // let hrot = Quat::IDENTITY;
                self.cam_quat = (hrot * (self.cam_quat * vrot)).normalize();
            }
            self.view_mat = Mat4::from_quat(self.cam_quat.inverse()) * Mat4::from_translation(-self.cam_pos);
        }
    }

    pub fn handle_action_bin(&mut self) {
        if let Some(action) = self.action_bin.dispatch() {
            self.dispatch_action(action);
        }
    }

    pub fn dispatch_action(&mut self, action: Action) {
        match action {
            Action::None => {}
            Action::Quit => {
                self.is_running = false;
            }
            Action::ToggleUI => {
                self.is_gui = !self.is_gui;
                if self.is_gui {
                    // reset ui origin when gui is turned on
                    self.ui_origin = reset_origin(self.current_camera_mat());
                    // cancel movement if switched to gui
                    self.move_forward = false;
                    self.move_backward = false;
                    self.move_left = false;
                    self.move_right = false;
                }
            }
            Action::ResetWorldOrigin => {
                self.world_origin = reset_origin(self.current_camera_mat());
            }
            Action::Command(cmd) => {
                let s = cmd.iter().map(|v| v.as_str()).collect::<Vec<_>>();
                self.mpv.command_async(&s);
            }
        }
    }

    pub fn wait_get_hmd_pose(&mut self) {
        if let (Some(vr), Some(vr_info)) = (&self.vr, &mut self.vr_info) {
            let m = vr.compositor.wait_get_hmd_pose();
            vr_info.orig_hmd_mat = m;
            vr_info.hmd_mat = (self.swap_z * m * self.swap_z).inverse();
        }
    }

    pub fn shutdown(&mut self) {
        self.gpu.shutdown();
        self.shared_tex.shutdown(&self.gpu.ash_device);
        if let Some(vr_ctx) = &self.vr {
            vr_ctx.shutdown();
        }
    }

    pub fn run(&mut self) {
        while self.is_running {
            self.main_loop();
        }
        self.shutdown();
    }
}
