use std::{
    ffi::{c_void, CStr},
    ptr,
};

use khronos_egl::{Context, Display, DynamicInstance, Surface};

pub fn get_gl_string(name: gl::types::GLenum) -> &'static str {
    unsafe { CStr::from_ptr(gl::GetString(name) as *const i8).to_str().unwrap() }
}

pub struct EGLContext {
    pub egl: DynamicInstance<khronos_egl::EGL1_2>,
    pub display: Display,
    pub context: Context,
    pub surface: Surface,
}

impl Drop for EGLContext {
    fn drop(&mut self) {
        self.egl.destroy_surface(self.display, self.surface).unwrap();
        self.egl.destroy_context(self.display, self.context).unwrap();
    }
}

pub fn load_egl() -> Box<EGLContext> {
    let egl = unsafe { khronos_egl::DynamicInstance::<khronos_egl::EGL1_2>::load_required().unwrap() };
    let display = egl.get_display(khronos_egl::DEFAULT_DISPLAY).unwrap();

    let version = egl.initialize(display).unwrap();
    log::info!("egl version: {}.{}", version.0, version.1);

    let config = egl
        .choose_first_config(
            display,
            &[khronos_egl::RENDERABLE_TYPE, khronos_egl::OPENGL_BIT, khronos_egl::NONE],
        )
        .unwrap()
        .unwrap();

    egl.bind_api(khronos_egl::OPENGL_API).unwrap();

    let context = egl
        .create_context(
            display,
            config,
            None,
            &[
                khronos_egl::CONTEXT_OPENGL_PROFILE_MASK,
                khronos_egl::CONTEXT_OPENGL_CORE_PROFILE_BIT,
                khronos_egl::NONE,
            ],
        )
        .unwrap();

    let surface = egl
        .create_pbuffer_surface(
            display,
            config,
            &[khronos_egl::WIDTH, 10, khronos_egl::HEIGHT, 10, khronos_egl::NONE],
        )
        .unwrap();

    egl.make_current(display, Some(surface), Some(surface), Some(context))
        .unwrap();

    gl::load_with(|s| {
        egl.get_proc_address(s)
            .map(|v| v as *const c_void)
            .unwrap_or(ptr::null())
    });

    log::info!("gl vendor: {}", get_gl_string(gl::VENDOR));
    log::info!("gl renderer: {}", get_gl_string(gl::RENDERER));
    log::info!("gl version: {}", get_gl_string(gl::VERSION));

    Box::new(EGLContext {
        egl,
        context,
        surface,
        display,
    })
}
