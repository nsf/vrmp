use khronos_egl::DynamicInstance;
use std::{
    collections::HashMap,
    ffi::{c_void, CStr, CString},
    mem::MaybeUninit,
    os::raw::{c_char, c_int},
    ptr,
    sync::Mutex,
};

extern "C" fn gl_get_proc_address(ctx: *mut c_void, name: *const c_char) -> *mut c_void {
    unsafe {
        let ctx = &*(ctx as *const DynamicInstance<khronos_egl::EGL1_2>);
        ctx.get_proc_address(CStr::from_ptr(name).to_str().unwrap())
            .map(|v| v as *mut c_void)
            .unwrap_or(ptr::null_mut())
    }
}
extern "C" fn on_mpv_events(ctx: *mut c_void) {
    unsafe {
        let ctx = &mut *(ctx as *mut Context);
        let mut has_events = ctx.has_events.lock().unwrap();
        *has_events = true;
    }
}
extern "C" fn on_mpv_render_update(ctx: *mut c_void) {
    unsafe {
        let ctx = &mut *(ctx as *mut RenderContext);
        let mut update_requested = ctx.update_requested.lock().unwrap();
        *update_requested = true;
    }
}

pub struct RenderContext {
    handle: *mut sys::mpv_render_context,
    update_requested: Mutex<bool>,
    redraw_requested: bool,
}

impl Drop for RenderContext {
    fn drop(&mut self) {
        unsafe {
            sys::mpv_render_context_free(self.handle);
            self.handle = ptr::null_mut();
        };
    }
}

impl RenderContext {
    pub fn update_maybe(&mut self) {
        {
            let mut update_requested = self.update_requested.lock().unwrap();
            if !*update_requested {
                return;
            }
            *update_requested = false;
        }
        unsafe {
            let flags = sys::mpv_render_context_update(self.handle);
            if flags & (sys::MPV_RENDER_UPDATE_FRAME as u64) != 0 {
                self.redraw_requested = true;
            }
        }
    }

    pub fn render_maybe(&mut self, fbo: c_int, w: c_int, h: c_int, internal_format: c_int) -> bool {
        if !self.redraw_requested {
            return false;
        }
        self.redraw_requested = false;
        unsafe {
            let mut opengl_fbo = sys::mpv_opengl_fbo {
                fbo,
                w,
                h,
                internal_format,
            };
            let mut params = [
                sys::mpv_render_param {
                    type_: sys::MPV_RENDER_PARAM_OPENGL_FBO,
                    data: &mut opengl_fbo as *mut sys::mpv_opengl_fbo as *mut c_void,
                },
                sys::mpv_render_param {
                    type_: sys::MPV_RENDER_PARAM_BLOCK_FOR_TARGET_TIME,
                    data: &mut 0 as *mut c_int as *mut c_void,
                },
                sys::mpv_render_param {
                    type_: sys::MPV_RENDER_PARAM_INVALID,
                    data: ptr::null_mut(),
                },
            ];
            sys::mpv_render_context_render(self.handle, &mut params[0]);
        }
        true
    }
}

pub struct Context {
    handle: *mut sys::mpv_handle,
    has_events: Mutex<bool>,
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            sys::mpv_terminate_destroy(self.handle);
            self.handle = ptr::null_mut();
        };
    }
}

pub enum Event {
    Property(Property),
    PropertyChange(String),
    VideoReconfig,
    FileLoaded,
    EndFile,
}

#[derive(Debug)]
pub enum Node {
    I64(i64),
    F64(f64),
    Bool(bool),
    String(String),
    Array(Vec<Node>),
    Map(HashMap<String, Node>),
}

impl Node {
    pub fn as_i64(&self) -> Option<&i64> {
        if let Node::I64(v) = &self {
            Some(v)
        } else {
            None
        }
    }
    pub fn as_f64(&self) -> Option<&f64> {
        if let Node::F64(v) = &self {
            Some(v)
        } else {
            None
        }
    }
    pub fn as_bool(&self) -> Option<&bool> {
        if let Node::Bool(v) = &self {
            Some(v)
        } else {
            None
        }
    }
    pub fn as_string(&self) -> Option<&String> {
        if let Node::String(v) = &self {
            Some(v)
        } else {
            None
        }
    }
    pub fn as_array(&self) -> Option<&Vec<Node>> {
        if let Node::Array(v) = &self {
            Some(v)
        } else {
            None
        }
    }
    pub fn as_map(&self) -> Option<&HashMap<String, Node>> {
        if let Node::Map(v) = &self {
            Some(v)
        } else {
            None
        }
    }
}

pub enum PropertyValue {
    I64(i64),
    F64(f64),
    Bool(bool),
    String(String),
    Node(Node),
}

pub struct Property {
    pub version: u64,
    pub name: String,
    pub value: PropertyValue,
}

fn convert_node(n: *const sys::mpv_node) -> Option<Node> {
    unsafe {
        match (*n).format {
            sys::MPV_FORMAT_STRING => {
                let v = (*n).u.string;
                Some(Node::String(if v != ptr::null_mut() {
                    CStr::from_ptr(v).to_string_lossy().to_string()
                } else {
                    String::new()
                }))
            }
            sys::MPV_FORMAT_FLAG => Some(Node::Bool((*n).u.flag != 0)),
            sys::MPV_FORMAT_INT64 => Some(Node::I64((*n).u.int64)),
            sys::MPV_FORMAT_DOUBLE => Some(Node::F64((*n).u.double_)),
            sys::MPV_FORMAT_NODE_ARRAY => {
                let num = (*(*n).u.list).num;
                let mut out = Vec::with_capacity(num as usize);
                let mut ptr = (*(*n).u.list).values;
                for _ in 0..num {
                    let val = match convert_node(ptr) {
                        Some(val) => val,
                        None => {
                            ptr = ptr.add(1);
                            continue;
                        }
                    };
                    out.push(val);
                    ptr = ptr.add(1);
                }
                Some(Node::Array(out))
            }
            sys::MPV_FORMAT_NODE_MAP => {
                let num = (*(*n).u.list).num;
                let mut out = HashMap::with_capacity(num as usize);
                let mut pkey = (*(*n).u.list).keys;
                let mut pval = (*(*n).u.list).values;
                for _ in 0..num {
                    let val = match convert_node(pval) {
                        Some(val) => val,
                        None => {
                            pkey = pkey.add(1);
                            pval = pval.add(1);
                            continue;
                        }
                    };
                    let key = if *pkey != ptr::null_mut() {
                        CStr::from_ptr(*pkey).to_string_lossy().to_string()
                    } else {
                        String::new()
                    };
                    out.insert(key, val);
                    pkey = pkey.add(1);
                    pval = pval.add(1);
                }
                Some(Node::Map(out))
            }
            _ => None,
        }
    }
}

impl Context {
    pub fn create() -> Box<Context> {
        unsafe {
            let handle = sys::mpv_create();
            if handle == ptr::null_mut() {
                panic!("mpv_create() failed");
            }

            let mut ctx = Box::new(Context {
                handle,
                has_events: Mutex::new(false),
            });

            sys::mpv_set_wakeup_callback(handle, Some(on_mpv_events), ctx.as_mut() as *mut Context as *mut c_void);

            sys::mpv_request_log_messages(handle, "debug\0".as_ptr() as *const i8);

            sys::mpv_set_option_string(handle, "hwdec\0".as_ptr() as *const i8, "no\0".as_ptr() as *const i8);
            sys::mpv_set_option_string(
                handle,
                "profile\0".as_ptr() as *const i8,
                "sw-fast\0".as_ptr() as *const i8,
            );

            ctx
        }
    }

    pub fn initialize(&self) {
        unsafe {
            if sys::mpv_initialize(self.handle) < 0 {
                panic!("mpv_initialize() failed");
            }
        }
    }

    pub fn command_async(&self, args: &[&str]) {
        unsafe {
            let args = args.iter().map(|&s| CString::new(s).unwrap()).collect::<Vec<_>>();
            let mut c_args = args.iter().map(|s| s.as_ptr()).collect::<Vec<_>>();
            c_args.push(ptr::null());
            sys::mpv_command_async(self.handle, 0, c_args.as_mut_ptr());
        }
    }

    pub fn observe_property(&self, name: &str) {
        unsafe {
            let cstr = CString::new(name).unwrap();
            sys::mpv_observe_property(self.handle, 0, cstr.as_ptr(), sys::MPV_FORMAT_NONE);
        }
    }

    fn get_property_async(&self, name: *const i8, format: sys::mpv_format) {
        unsafe {
            sys::mpv_get_property_async(self.handle, 0, name, format);
        }
    }

    pub fn get_size_async(&self) {
        self.get_property_async("width\0".as_ptr() as *const i8, sys::MPV_FORMAT_INT64);
        self.get_property_async("height\0".as_ptr() as *const i8, sys::MPV_FORMAT_INT64);
    }

    pub fn get_percent_pos_async(&self) {
        self.get_property_async("percent-pos\0".as_ptr() as *const i8, sys::MPV_FORMAT_DOUBLE)
    }

    pub fn get_duration_async(&self) {
        self.get_property_async("duration\0".as_ptr() as *const i8, sys::MPV_FORMAT_INT64);
    }

    pub fn get_pause_async(&self) {
        self.get_property_async("pause\0".as_ptr() as *const i8, sys::MPV_FORMAT_FLAG);
    }

    pub fn get_hwdec_async(&self) {
        self.get_property_async("hwdec\0".as_ptr() as *const i8, sys::MPV_FORMAT_STRING);
    }

    pub fn get_hwdec_current_async(&self) {
        self.get_property_async("hwdec-current\0".as_ptr() as *const i8, sys::MPV_FORMAT_STRING);
    }

    pub fn get_path_async(&self) {
        self.get_property_async("path\0".as_ptr() as *const i8, sys::MPV_FORMAT_STRING);
    }

    pub fn get_video_params_async(&self) {
        self.get_property_async("video-params\0".as_ptr() as *const i8, sys::MPV_FORMAT_NODE);
    }

    pub fn get_track_list_async(&self) {
        self.get_property_async("track-list\0".as_ptr() as *const i8, sys::MPV_FORMAT_NODE);
    }

    pub fn get_vid_async(&self) {
        self.get_property_async("vid\0".as_ptr() as *const i8, sys::MPV_FORMAT_INT64);
    }

    pub fn get_sid_async(&self) {
        self.get_property_async("sid\0".as_ptr() as *const i8, sys::MPV_FORMAT_INT64);
    }

    pub fn get_aid_async(&self) {
        self.get_property_async("aid\0".as_ptr() as *const i8, sys::MPV_FORMAT_INT64);
    }

    pub fn drain_events(&mut self) -> Vec<Event> {
        {
            let mut has_events = self.has_events.lock().unwrap();
            if !*has_events {
                return vec![];
            }
            *has_events = false;
        }
        let mut events = Vec::new();
        let mut video_reconfig = false;
        let mut file_loaded = false;
        let mut end_file = false;
        unsafe {
            loop {
                let event = sys::mpv_wait_event(self.handle, 0.0);
                if (*event).event_id == sys::MPV_EVENT_NONE {
                    break;
                } else if (*event).event_id == sys::MPV_EVENT_LOG_MESSAGE {
                    let log_message = (*event).data as *const sys::mpv_event_log_message;
                    let level = (*log_message).log_level;
                    let prefix = CStr::from_ptr((*log_message).prefix).to_str().unwrap();
                    let mut text = CStr::from_ptr((*log_message).text).to_str().unwrap();
                    if text.ends_with("\n") {
                        text = &text[..text.len() - 1];
                    }
                    match level {
                        // "fatal" - critical/aborting errors
                        sys::MPV_LOG_LEVEL_FATAL => log::error!("[{}] {}", prefix, text),
                        // "error" - simple errors
                        sys::MPV_LOG_LEVEL_ERROR => log::error!("[{}] {}", prefix, text),
                        // "warn"  - possible problems
                        sys::MPV_LOG_LEVEL_WARN => log::warn!("[{}] {}", prefix, text),
                        // "info"  - informational message
                        sys::MPV_LOG_LEVEL_INFO => log::info!("[{}] {}", prefix, text),
                        // "v"     - noisy informational message
                        sys::MPV_LOG_LEVEL_V => log::info!("[{}] {}", prefix, text),
                        // "debug" - very noisy technical information
                        sys::MPV_LOG_LEVEL_DEBUG => log::debug!("[{}] {}", prefix, text),
                        // "trace" - extremely noisy
                        sys::MPV_LOG_LEVEL_TRACE => log::trace!("[{}] {}", prefix, text),
                        _ => {}
                    }
                } else if (*event).event_id == sys::MPV_EVENT_VIDEO_RECONFIG {
                    video_reconfig = true;
                } else if (*event).event_id == sys::MPV_EVENT_END_FILE {
                    end_file = true
                } else if (*event).event_id == sys::MPV_EVENT_FILE_LOADED {
                    file_loaded = true;
                } else if (*event).event_id == sys::MPV_EVENT_PROPERTY_CHANGE {
                    let ep = (*event).data as *const sys::mpv_event_property;
                    let name = CStr::from_ptr((*ep).name).to_str().unwrap().to_owned();
                    if (*ep).format != sys::MPV_FORMAT_NONE {
                        log::warn!("unexpected property change event for: {}", &name);
                        continue;
                    }
                    events.push(Event::PropertyChange(name));
                } else if (*event).event_id == sys::MPV_EVENT_GET_PROPERTY_REPLY {
                    let ep = (*event).data as *const sys::mpv_event_property;
                    if (*ep).format == sys::MPV_FORMAT_NONE {
                        continue;
                    }
                    let version = (*event).reply_userdata;
                    let name = CStr::from_ptr((*ep).name).to_str().unwrap().to_owned();
                    let data = (*ep).data;
                    match (*ep).format {
                        sys::MPV_FORMAT_INT64 => events.push(Event::Property(Property {
                            version,
                            name,
                            value: PropertyValue::I64(*(data as *const i64)),
                        })),
                        sys::MPV_FORMAT_DOUBLE => events.push(Event::Property(Property {
                            version,
                            name,
                            value: PropertyValue::F64(*(data as *const f64)),
                        })),
                        sys::MPV_FORMAT_FLAG => events.push(Event::Property(Property {
                            version,
                            name,
                            value: PropertyValue::Bool(*(data as *const c_int) != 0),
                        })),
                        sys::MPV_FORMAT_NODE => {
                            if let Some(node) = convert_node(data as *const sys::mpv_node) {
                                events.push(Event::Property(Property {
                                    version,
                                    name,
                                    value: PropertyValue::Node(node),
                                }));
                            }
                        }
                        sys::MPV_FORMAT_STRING => {
                            let cstr = *(data as *const *const c_char);
                            let v = if cstr != ptr::null_mut() {
                                CStr::from_ptr(cstr).to_string_lossy().to_string()
                            } else {
                                String::new()
                            };
                            events.push(Event::Property(Property {
                                version,
                                name,
                                value: PropertyValue::String(v),
                            }));
                        }
                        _ => {}
                    }
                } else {
                    let event_name = CStr::from_ptr(sys::mpv_event_name((*event).event_id)).to_str().unwrap();
                    log::info!("event: {}", event_name);
                }
            }
        }
        if file_loaded {
            events.push(Event::FileLoaded);
        }
        if video_reconfig {
            events.push(Event::VideoReconfig);
        }
        if end_file {
            events.push(Event::EndFile);
        }
        events
    }

    pub unsafe fn create_render_context(
        &self,
        egl: &DynamicInstance<khronos_egl::EGL1_2>,
        window: &sdl2::video::Window,
    ) -> Box<RenderContext> {
        let version = sdl2::version::version();
        let mut wminfo: sdl2_sys::SDL_SysWMinfo = MaybeUninit::zeroed().assume_init();
        wminfo.version.major = version.major;
        wminfo.version.minor = version.minor;
        wminfo.version.patch = version.patch;
        if sdl2_sys::SDL_GetWindowWMInfo(window.raw(), &mut wminfo) != sdl2_sys::SDL_bool::SDL_TRUE {
            panic!("SDL_GetWindowWMInfo failed");
        }
        if wminfo.subsystem != sdl2_sys::SDL_SYSWM_TYPE::SDL_SYSWM_X11 {
            panic!("not X11 system");
        }

        let mut handle: *mut sys::mpv_render_context = ptr::null_mut();

        let mut opengl_params = sys::mpv_opengl_init_params {
            get_proc_address: Some(gl_get_proc_address),
            get_proc_address_ctx: egl as *const DynamicInstance<khronos_egl::EGL1_2> as *mut c_void,
            extra_exts: "\0".as_ptr() as *const i8,
        };

        let mut params = [
            sys::mpv_render_param {
                type_: sys::MPV_RENDER_PARAM_X11_DISPLAY,
                data: wminfo.info.x11.display as *mut c_void,
            },
            sys::mpv_render_param {
                type_: sys::MPV_RENDER_PARAM_API_TYPE,
                data: sys::MPV_RENDER_API_TYPE_OPENGL.as_ptr() as *mut c_void,
            },
            sys::mpv_render_param {
                type_: sys::MPV_RENDER_PARAM_OPENGL_INIT_PARAMS,
                data: &mut opengl_params as *mut sys::mpv_opengl_init_params as *mut c_void,
            },
            sys::mpv_render_param {
                type_: sys::MPV_RENDER_PARAM_ADVANCED_CONTROL,
                data: &mut 1 as *mut c_int as *mut c_void,
            },
            sys::mpv_render_param {
                type_: sys::MPV_RENDER_PARAM_INVALID,
                data: ptr::null_mut(),
            },
        ];

        let result = sys::mpv_render_context_create(&mut handle, self.handle, &mut params[0]);
        if result < 0 {
            panic!("mpv_render_context_create() failed: {}", result);
        }
        let mut ctx = Box::new(RenderContext {
            handle,
            update_requested: Mutex::new(false),
            redraw_requested: false,
        });
        sys::mpv_render_context_set_update_callback(
            handle,
            Some(on_mpv_render_update),
            ctx.as_mut() as *mut RenderContext as *mut c_void,
        );
        ctx
    }
}
