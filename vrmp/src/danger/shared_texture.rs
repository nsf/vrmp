use std::{ptr, sync::Arc};

use ash::vk;

use super::{opengl::OpenGLSharedTexture, vulkan::VulkanSharedTexture};

// Garbage items will be destroyed after this number of frames. It's implied that they are not used during that period.
const DESTROY_AFTER_NUM_FRAMES: u32 = 60;

struct Garbage {
    frame_lifetime: u32,
    vk: VulkanSharedTexture,
    gl: OpenGLSharedTexture,
}

pub struct SharedTexture {
    pub vk: VulkanSharedTexture,
    pub gl: OpenGLSharedTexture,

    ready: bool,
    gl_did_draw: bool,
    // this is a list of textures to destroy, I don't properly wait on a fence to destroy it, just delay destruction by
    // a couple of frames after use
    garbage: Vec<Garbage>,
    resize_requested: Option<(u32, u32)>,
}

impl SharedTexture {
    pub fn create(
        instance: &ash::Instance,
        device: &ash::Device,
        physical_device: vk::PhysicalDevice,
        wgpu_device: &wgpu::Device,
        bind_group_layout: Arc<wgpu::BindGroupLayout>,
        w: u32,
        h: u32,
    ) -> SharedTexture {
        unsafe {
            let vk =
                VulkanSharedTexture::create(instance, device, physical_device, wgpu_device, bind_group_layout, w, h);
            let gl = OpenGLSharedTexture::create(&vk);
            SharedTexture {
                ready: false,
                vk,
                gl,
                gl_did_draw: false,
                garbage: Vec::new(),
                resize_requested: None,
            }
        }
    }

    pub fn is_ready(&self) -> bool {
        self.ready
    }

    pub fn request_resize(&mut self, w: u32, h: u32) {
        if (self.vk.width != w || self.vk.height != h) && w != 0 && h != 0 {
            self.resize_requested = Some((w, h));
        }
    }

    pub fn resize_maybe(
        &mut self,
        instance: &ash::Instance,
        device: &ash::Device,
        physical_device: vk::PhysicalDevice,
        wgpu_device: &wgpu::Device,
    ) {
        unsafe {
            if let Some((w, h)) = self.resize_requested {
                log::info!("resizing shared texture to {}x{}", w, h);
                self.resize_requested = None;
                let new_vk = VulkanSharedTexture::create(
                    instance,
                    device,
                    physical_device,
                    wgpu_device,
                    self.vk.bind_group_layout.clone(),
                    w,
                    h,
                );
                let new_gl = OpenGLSharedTexture::create(&new_vk);
                let old_vk = std::mem::replace(&mut self.vk, new_vk);
                let old_gl = std::mem::replace(&mut self.gl, new_gl);

                // put the thing into "garbage", it will be destroyed few frames later
                self.garbage.push(Garbage {
                    frame_lifetime: 0,
                    vk: old_vk,
                    gl: old_gl,
                });

                // since it's a new semaphore, we don't need to wait on it, GL will draw something next frame
                self.gl_did_draw = false;
                self.ready = false;
            }
        }
    }

    pub fn before_vk(&mut self, device: &ash::Device, queue: vk::Queue) {
        // wait on semaphore only if GL did draw something
        if self.gl_did_draw {
            self.gl_did_draw = false;

            unsafe {
                let vk_info = vk::SubmitInfo::builder()
                    .wait_dst_stage_mask(&[vk::PipelineStageFlags::FRAGMENT_SHADER])
                    .wait_semaphores(&[self.vk.gl_complete])
                    .build();
                device.queue_submit(queue, &[vk_info], vk::Fence::null()).unwrap();
            }
        }
    }

    pub fn after_vk(&mut self, device: &ash::Device, queue: vk::Queue) {
        // vk always signals a semaphore
        unsafe {
            let vk_info = vk::SubmitInfo::builder().signal_semaphores(&[self.vk.gl_ready]).build();
            device.queue_submit(queue, &[vk_info], vk::Fence::null()).unwrap();
        }

        // destroy garbage if any
        let mut i = 0;
        while i < self.garbage.len() {
            let item = &mut self.garbage[i];
            item.frame_lifetime += 1;
            let to_be_destroyed = item.frame_lifetime > DESTROY_AFTER_NUM_FRAMES;
            if to_be_destroyed {
                let item = self.garbage.remove(i);
                log::info!("destroying garbage shared texture {}x{}", item.vk.width, item.vk.height);
                item.vk.shutdown(device);
                item.gl.shutdown();
            } else {
                i += 1;
            }
        }
    }

    pub fn shutdown(&self, device: &ash::Device) {
        self.vk.shutdown(device);
        self.gl.shutdown();
    }

    pub fn draw_gl<F: FnOnce() -> bool>(&mut self, f: F) {
        unsafe {
            let x = gl::LAYOUT_SHADER_READ_ONLY_EXT;
            let y = gl::LAYOUT_SHADER_READ_ONLY_EXT;
            gl::WaitSemaphoreEXT(self.gl.gl_ready, 0, ptr::null(), 1, &self.gl.gl_texture, &x);

            gl::Viewport(0, 0, self.gl.width as i32, self.gl.height as i32);
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.gl.gl_fbo);

            let did_draw = f();
            if did_draw {
                self.ready = true;
            }

            gl::SignalSemaphoreEXT(self.gl.gl_complete, 0, ptr::null(), 1, &self.gl.gl_texture, &y);

            gl::Flush();
        }
        self.gl_did_draw = true;
    }
}
