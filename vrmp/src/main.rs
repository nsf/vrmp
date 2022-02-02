#[macro_export]
macro_rules! include_shader {
    ($path:literal) => {
        include_str!(concat!(env!("OUT_DIR"), "/shaders/", $path))
    };
}

#[macro_export]
macro_rules! cond {
    ($cond:expr, $case_true: expr, $case_false: expr) => {
        if $cond {
            $case_true
        } else {
            $case_false
        }
    };
}

mod action;
mod buflog;
mod camera_state;
mod config;
mod controls;
mod danger;
mod enums;
mod filedb;
mod global;
mod imgui;
mod multilog;
mod pipeline;
mod scene;
mod tracks;
mod vrinfo;
mod vscreen;

fn main() {
    env_logger::init();
    let mut global = global::Global::init();
    global.run();
}
