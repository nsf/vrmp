use std::time::Instant;

use imgui::{BackendFlags, Key};
use sdl2::{
    event::Event,
    keyboard::{Mod, Scancode},
    mouse::MouseButton,
};

// virtual screen for UI

pub struct VScreen {
    pub texture: wgpu::Texture,
    pub texture_view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
    pub width: u32,
    pub height: u32,
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub last_frame: Instant,
    pub mouse_buttons: [Button; 5],
}

impl VScreen {
    pub fn create(device: &wgpu::Device, bind_group_layout: &wgpu::BindGroupLayout, w: u32, h: u32) -> VScreen {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: w,
                height: h,
                ..Default::default()
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            }],
            label: None,
        });

        VScreen {
            texture,
            texture_view,
            bind_group,
            width: w,
            height: h,
            mouse_x: w as f32 / 2.0,
            mouse_y: h as f32 / 2.0,
            last_frame: Instant::now(),
            mouse_buttons: [Button::new(); 5],
        }
    }

    pub fn imgui_init(&mut self, imgui: &mut imgui::Context) {
        let io = imgui.io_mut();

        io.backend_flags.insert(BackendFlags::HAS_SET_MOUSE_POS);

        io[Key::Tab] = Scancode::Tab as _;
        io[Key::LeftArrow] = Scancode::Left as _;
        io[Key::RightArrow] = Scancode::Right as _;
        io[Key::UpArrow] = Scancode::Up as _;
        io[Key::DownArrow] = Scancode::Down as _;
        io[Key::PageUp] = Scancode::PageUp as _;
        io[Key::PageDown] = Scancode::PageDown as _;
        io[Key::Home] = Scancode::Home as _;
        io[Key::End] = Scancode::End as _;
        io[Key::Insert] = Scancode::Insert as _;
        io[Key::Delete] = Scancode::Delete as _;
        io[Key::Backspace] = Scancode::Backspace as _;
        io[Key::Space] = Scancode::Space as _;
        io[Key::Enter] = Scancode::Return as _;
        io[Key::Escape] = Scancode::Escape as _;
        io[Key::KeyPadEnter] = Scancode::KpEnter as _;
        io[Key::A] = Scancode::A as _;
        io[Key::C] = Scancode::C as _;
        io[Key::V] = Scancode::V as _;
        io[Key::X] = Scancode::X as _;
        io[Key::Y] = Scancode::Y as _;
        io[Key::Z] = Scancode::Z as _;

        imgui.set_platform_name(Some(format!("vrmp virtual screen")));
    }

    fn handle_mouse_button(&mut self, button: &MouseButton, pressed: bool) {
        match button {
            MouseButton::Left => self.mouse_buttons[0].set(pressed),
            MouseButton::Right => self.mouse_buttons[1].set(pressed),
            MouseButton::Middle => self.mouse_buttons[2].set(pressed),
            MouseButton::X1 => self.mouse_buttons[3].set(pressed),
            MouseButton::X2 => self.mouse_buttons[4].set(pressed),

            _ => {}
        }
    }

    pub fn imgui_handle_event(&mut self, context: &mut imgui::Context, event: &Event) {
        let io = context.io_mut();

        match *event {
            Event::MouseMotion { xrel, yrel, .. } => {
                self.mouse_x += xrel as f32;
                self.mouse_y += yrel as f32;
            }

            Event::MouseWheel { x, y, .. } => {
                io.mouse_wheel = y as f32;
                io.mouse_wheel_h = x as f32;
            }

            Event::MouseButtonDown { mouse_btn, .. } => {
                self.handle_mouse_button(&mouse_btn, true);
            }

            Event::MouseButtonUp { mouse_btn, .. } => {
                self.handle_mouse_button(&mouse_btn, false);
            }

            Event::TextInput { ref text, .. } => {
                text.chars().for_each(|c| io.add_input_character(c));
            }

            Event::KeyDown {
                scancode: Some(key),
                keymod,
                ..
            } => {
                io.keys_down[key as usize] = true;
                handle_key_modifier(io, &keymod);
            }

            Event::KeyUp {
                scancode: Some(key),
                keymod,
                ..
            } => {
                io.keys_down[key as usize] = false;
                handle_key_modifier(io, &keymod);
            }

            _ => {}
        }
    }

    pub fn imgui_prepare_frame(&mut self, context: &mut imgui::Context) {
        let io = context.io_mut();
        let now = Instant::now();
        io.update_delta_time(now.duration_since(self.last_frame));
        self.last_frame = now;

        io.display_size = [self.width as f32, self.height as f32];
        io.display_framebuffer_scale = [1.0, 1.0];

        for (io_down, button) in io.mouse_down.iter_mut().zip(&mut self.mouse_buttons) {
            *io_down = button.get();
            button.pressed_this_frame = false;
        }

        if io.want_set_mouse_pos {
            self.mouse_x = io.mouse_pos[0];
            self.mouse_y = io.mouse_pos[1];
        }
        io.mouse_pos = [self.mouse_x, self.mouse_y];
    }
}

fn handle_key_modifier(io: &mut imgui::Io, keymod: &Mod) {
    io.key_shift = keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD);
    io.key_ctrl = keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD);
    io.key_alt = keymod.intersects(Mod::LALTMOD | Mod::RALTMOD);
    io.key_super = keymod.intersects(Mod::LGUIMOD | Mod::RGUIMOD);
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Button {
    pub pressed_this_frame: bool,
    pub state: bool,
}

impl Button {
    const fn new() -> Button {
        Button {
            pressed_this_frame: false,
            state: false,
        }
    }

    fn get(&self) -> bool {
        self.pressed_this_frame || self.state
    }

    fn set(&mut self, pressed: bool) {
        self.state = pressed;

        if pressed {
            self.pressed_this_frame = true;
        }
    }
}
