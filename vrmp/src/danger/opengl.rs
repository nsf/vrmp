use std::{
    ffi::{c_void, CString},
    ptr,
    time::Instant,
};

use super::vulkan::VulkanSharedTexture;

pub struct OpenGLSharedTexture {
    pub gl_texture: u32,
    pub gl_ready: u32,
    pub gl_complete: u32,
    pub gl_memory: u32,
    pub gl_fbo: u32,
    pub width: u32,
    pub height: u32,
}

impl OpenGLSharedTexture {
    pub unsafe fn create(vk: &VulkanSharedTexture) -> OpenGLSharedTexture {
        let mut gl_texture = 0u32;
        let mut gl_ready = 0u32;
        let mut gl_complete = 0u32;
        let mut gl_memory = 0u32;
        let mut gl_fbo = 0u32;

        // create objects
        gl::GenTextures(1, &mut gl_texture);
        gl::GenSemaphoresEXT(1, &mut gl_ready);
        gl::GenSemaphoresEXT(1, &mut gl_complete);
        gl::CreateMemoryObjectsEXT(1, &mut gl_memory);
        gl::GenFramebuffers(1, &mut gl_fbo);

        // import FDs
        gl::ImportSemaphoreFdEXT(gl_ready, gl::HANDLE_TYPE_OPAQUE_FD_EXT, vk.gl_ready_fd);
        gl::ImportSemaphoreFdEXT(gl_complete, gl::HANDLE_TYPE_OPAQUE_FD_EXT, vk.gl_complete_fd);
        gl::ImportMemoryFdEXT(
            gl_memory,
            vk.memory_size,
            gl::HANDLE_TYPE_OPAQUE_FD_EXT,
            vk.gl_memory_fd,
        );

        // apply memory storage to texture
        gl::BindTexture(gl::TEXTURE_2D, gl_texture);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_TILING_EXT, gl::LINEAR_TILING_EXT as i32);
        gl::TexStorageMem2DEXT(
            gl::TEXTURE_2D,
            1,
            gl::SRGB8,
            vk.width as i32,
            vk.height as i32,
            gl_memory,
            0,
        );

        gl::BindFramebuffer(gl::FRAMEBUFFER, gl_fbo);
        gl::FramebufferTexture(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, gl_texture, 0);
        OpenGLSharedTexture {
            gl_texture,
            gl_ready,
            gl_complete,
            gl_memory,
            gl_fbo,
            width: vk.width,
            height: vk.height,
        }
    }

    pub fn shutdown(&self) {
        unsafe {
            gl::DeleteFramebuffers(1, &self.gl_fbo);
            gl::DeleteTextures(1, &self.gl_texture);
            gl::DeleteMemoryObjectsEXT(1, &self.gl_memory);
            gl::DeleteSemaphoresEXT(1, &self.gl_ready);
            gl::DeleteSemaphoresEXT(1, &self.gl_complete);
        }
    }
}

// just a basic test code that draws something on the screen, it works, useful for testing opengl rendering.
pub struct OpenGLTestDrawData {
    pub vbo: u32,
    pub vao: u32,
    pub vert_shader: u32,
    pub frag_shader: u32,
    pub program: u32,
    time: Instant,
}

impl OpenGLTestDrawData {
    pub unsafe fn create() -> OpenGLTestDrawData {
        let vertices: [f32; 9] = [-0.5, -0.5, 0.0, 0.5, -0.5, 0.0, 0.0, 0.5, 0.0];
        let mut vbo = 0u32;
        let mut vao = 0u32;
        gl::GenVertexArrays(1, &mut vao);
        gl::BindVertexArray(vao);
        gl::GenBuffers(1, &mut vbo);
        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            (std::mem::size_of::<f32>() * 9) as isize,
            &vertices[0] as *const f32 as *const c_void,
            gl::STATIC_DRAW,
        );
        gl::EnableVertexAttribArray(0);
        gl::VertexAttribPointer(
            0,
            3,
            gl::FLOAT,
            gl::FALSE,
            (std::mem::size_of::<f32>() * 3) as i32,
            ptr::null(),
        );

        let vert_shader_source = CString::new(
            r#"
                #version 330 core
                layout (location = 0) in vec3 pos;
                void main()
                {
                    gl_Position = vec4(pos.x, pos.y, pos.z, 1.0);
                }
            "#,
        )
        .unwrap();
        let frag_shader_source = CString::new(
            r#"
                #version 330 core
                uniform float time;
                out vec4 FragColor;
                void main()
                {
                    FragColor = vec4(fract(time), 0.0f, 0.0f, 1.0f);
                }
            "#,
        )
        .unwrap();

        let vert_shader = gl::CreateShader(gl::VERTEX_SHADER);
        let frag_shader = gl::CreateShader(gl::FRAGMENT_SHADER);
        gl::ShaderSource(vert_shader, 1, &vert_shader_source.as_ptr(), ptr::null());
        gl::CompileShader(vert_shader);
        gl::ShaderSource(frag_shader, 1, &frag_shader_source.as_ptr(), ptr::null());
        gl::CompileShader(frag_shader);
        let program = gl::CreateProgram();
        gl::AttachShader(program, vert_shader);
        gl::AttachShader(program, frag_shader);
        gl::LinkProgram(program);

        //-----------------------------

        OpenGLTestDrawData {
            vbo,
            vao,
            vert_shader,
            frag_shader,
            program,
            time: Instant::now(),
        }
    }

    pub unsafe fn draw(&self) {
        gl::ClearColor(0.0, 0.1, 0.0, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);

        gl::UseProgram(self.program);
        gl::Uniform1f(0, self.time.elapsed().as_secs_f32());

        gl::BindVertexArray(self.vao);
        gl::DrawArrays(gl::TRIANGLES, 0, 3);
    }
}
