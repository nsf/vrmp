use std::fmt::Write;

use imgui::StyleColor;

use crate::action::{Action, ActionBin};
use crate::config::ConfigSyncer;
use crate::enums::{AspectRatio, Mode, Projection};
use crate::filedb::FileData;
use crate::tracks::{Track, Tracks};

use super::font_awesome as fa;

use super::util::{hex, iter_bit_spans};

use indoc::indoc;

pub struct General {
    pub percent_pos: f64,
    pub duration: u32,
    pub shader_debug: f32,
    pub show_demo: bool,
    pub playing: bool,
    pub hwdec: String,
    pub hwdec_current: String,

    tmp_str: String,
}

impl General {
    pub fn new() -> General {
        General {
            percent_pos: 0.0,
            duration: 0,
            shader_debug: 0.0,
            show_demo: false,
            playing: false,
            hwdec: String::new(),
            hwdec_current: String::new(),

            tmp_str: String::new(),
        }
    }

    pub fn render(
        &mut self,
        action_bin: &mut ActionBin,
        config_syncer: &mut ConfigSyncer,
        tracks: Option<&Tracks>,
        mut fdata: Option<&mut FileData>,
        ui: &imgui::Ui,
        position: [f32; 2],
        size: [f32; 2],
    ) {
        let window = imgui::Window::new("Video Settings");
        window
            .flags(imgui::WindowFlags::NO_RESIZE | imgui::WindowFlags::NO_TITLE_BAR)
            .position(position, imgui::Condition::FirstUseEver)
            .size(size, imgui::Condition::FirstUseEver)
            .build(ui, || {
                if ui.collapsing_header("Playback", imgui::TreeNodeFlags::DEFAULT_OPEN) {
                    let _token = ui.push_style_var(imgui::StyleVar::FramePadding([4.0, 15.0]));
                    if ui.button_with_size(cond!(self.playing, fa::PAUSE, fa::PLAY), [60.0, 0.0]) {
                        action_bin.put(Action::Command(vec!["cycle".to_owned(), "pause".to_owned()]));
                    }

                    ui.same_line();

                    let mut value = self.percent_pos;
                    ui.set_next_item_width(-1.0);
                    if imgui::Slider::new("##seek", 0.0, 100.0)
                        .display_format("")
                        .build(ui, &mut value)
                    {
                        if self.percent_pos != value {
                            action_bin.put(Action::Command(vec![
                                "seek".to_owned(),
                                format!("{}", value),
                                "absolute-percent".to_owned(),
                            ]));
                            self.percent_pos = value;
                        }
                    }
                    let [x0, _] = ui.item_rect_min();
                    let [_, y1] = ui.item_rect_max();
                    let [w, _] = ui.item_rect_size();
                    if ui.is_item_hovered() {
                        let [mx, _] = ui.io().mouse_pos;
                        let rx = mx - x0;
                        let fr = (rx / w.max(1.0)).clamp(0.0, 1.0); // clamp to be sure
                        let p = fr * 100.0;
                        let dur = std::time::Duration::from_secs(self.duration as u64);
                        let cdur = dur.mul_f32(fr);
                        let seconds = cdur.as_secs() % 60;
                        let minutes = (cdur.as_secs() / 60) % 60;
                        let hours = (cdur.as_secs() / 60) / 60;
                        let tmp_str = &mut self.tmp_str;
                        tmp_str.clear();
                        write!(tmp_str, "{:02}:{:02}:{:02} ({:.2}%)", hours, minutes, seconds, p).unwrap();
                        ui.tooltip_text(tmp_str);
                    }

                    if let Some(fdata) = fdata.as_deref_mut() {
                        let dl = ui.get_window_draw_list();
                        let caret_w = 10.0;
                        let caret_hw = caret_w / 2.0;
                        let some_padding = 2.0;
                        let line_h = 4.0;
                        let sec_w = (w - some_padding - some_padding - caret_w) / 128.0;
                        let base_x = some_padding + caret_hw + x0;
                        let base_y = y1 - some_padding - line_h;
                        let is_first = (fdata.seen0 & 1) != 0;
                        let is_last = ((fdata.seen1 >> 63) & 1) != 0;
                        if is_first {
                            dl.add_rect(
                                [x0 + some_padding, base_y],
                                [x0 + some_padding + caret_hw, base_y + line_h],
                                hex("#0372ff"),
                            )
                            .filled(true)
                            .build();
                        }
                        if is_last {
                            dl.add_rect(
                                [x0 + w - some_padding - caret_hw, base_y],
                                [x0 + w - some_padding, base_y + line_h],
                                hex("#0372ff"),
                            )
                            .filled(true)
                            .build();
                        }
                        iter_bit_spans(fdata.seen0, fdata.seen1, |b, e| {
                            dl.add_rect(
                                [base_x + sec_w * b as f32, base_y],
                                [base_x + sec_w * e as f32, base_y + line_h],
                                hex("#0372ff"),
                            )
                            .filled(true)
                            .build();
                        });
                    }
                }

                if ui.collapsing_header("Video Settings", imgui::TreeNodeFlags::empty()) {
                    // PROJECTION
                    if let Some(fdata) = fdata.as_deref_mut() {
                        ui.align_text_to_frame_padding();
                        ui.text("Projection:");
                        let mut projection_button = |label: &str, m: Projection, tooltip: &str| {
                            ui.same_line();
                            let _token = (fdata.projection == m).then(|| {
                                (
                                    ui.push_style_color(StyleColor::Button, hex("#816300")),
                                    ui.push_style_color(StyleColor::ButtonHovered, hex("#AE9400")),
                                )
                            });

                            if ui.button(label) {
                                fdata.projection = m;
                            }
                            if ui.is_item_hovered() {
                                ui.tooltip_text(tooltip);
                            }
                        };
                        projection_button("ER 360", Projection::Er360, "Equirectangular 360°");
                        projection_button("ER 180", Projection::Er180, "Equirectangular 180°");
                        projection_button("Fisheye", Projection::Fisheye, "Fisheye 180°");
                        projection_button("EAC", Projection::Eac, "Equi-Angular Cubemap");
                        projection_button("Flat", Projection::Flat, "Flat Screen");
                    }

                    // MODE
                    if let Some(fdata) = fdata.as_deref_mut() {
                        ui.align_text_to_frame_padding();
                        ui.text("Mode:");
                        let mut mode_button = |label: &str, m: Mode| {
                            ui.same_line();
                            let _token = (fdata.mode == m).then(|| {
                                (
                                    ui.push_style_color(StyleColor::Button, hex("#816300")),
                                    ui.push_style_color(StyleColor::ButtonHovered, hex("#AE9400")),
                                )
                            });
                            if ui.button(label) {
                                fdata.mode = m;
                            }
                        };
                        mode_button("Mono", Mode::Mono);
                        mode_button("Left/Right", Mode::LeftRight);
                        mode_button("Right/Left", Mode::RightLeft);
                        mode_button("Top/Bottom", Mode::TopBottom);
                        mode_button("Bottom/Top", Mode::BottomTop);
                        ui.same_line();

                        // let tmp_str = &mut self.tmp_str;
                        // tmp_str.clear();
                        // write!(tmp_str, "{} Swap Eyes", fa::EXCHANGE_ALT).unwrap();
                        if ui.button_with_size(fa::EXCHANGE_ALT, [80.0, 0.0]) {
                            fdata.flip_eyes();
                        }
                    }

                    // FLAT SCREEN
                    if let Some(fdata) = fdata.as_deref_mut() {
                        if fdata.projection == Projection::Flat {
                            ui.align_text_to_frame_padding();
                            ui.text("Screen Distance:");
                            ui.same_line();
                            {
                                let tmp_str = &mut self.tmp_str;
                                tmp_str.clear();
                                write!(tmp_str, "{:.3}m", fdata.flat_distance).unwrap();

                                imgui::Drag::new("##flat_distance")
                                    .range(0.2, 20.0)
                                    .speed(0.01)
                                    .display_format(tmp_str)
                                    .build(&ui, &mut fdata.flat_distance);
                            }
                            ui.align_text_to_frame_padding();
                            ui.text("Screen Scale:");
                            ui.same_line();
                            {
                                imgui::Drag::new("##flat_scale")
                                    .range(0.1, 10.0)
                                    .speed(0.01)
                                    .build(&ui, &mut fdata.flat_scale);
                            }
                        }
                    }

                    // ASPECT RATIO
                    if let Some(fdata) = fdata.as_deref_mut() {
                        if fdata.projection == Projection::Flat {
                            ui.align_text_to_frame_padding();
                            ui.text("Aspect Ratio:");
                            let mut aspect_button = |label: &str, v: AspectRatio| {
                                ui.same_line();
                                let _token = (fdata.aspect_ratio == v).then(|| {
                                    (
                                        ui.push_style_color(StyleColor::Button, hex("#816300")),
                                        ui.push_style_color(StyleColor::ButtonHovered, hex("#AE9400")),
                                    )
                                });
                                if ui.button(label) {
                                    fdata.aspect_ratio = v;
                                }
                            };
                            aspect_button("1/2", AspectRatio::Half);
                            aspect_button("1", AspectRatio::One);
                            aspect_button("2", AspectRatio::Two);
                        }
                    }

                    // ADJUST STEREO CONVERGENCE
                    if let Some(fdata) = fdata.as_deref_mut() {
                        ui.align_text_to_frame_padding();
                        ui.text("Stereo Convergence:");
                        ui.same_line();
                        ui.align_text_to_frame_padding();
                        ui.text_disabled(fa::QUESTION_CIRCLE);
                        if ui.is_item_hovered() {
                            ui.tooltip_text(indoc!(
                                r#"
                                Adjusts stereo convergence point. Effectively rotates picture in each eye inwards
                                (for positive value) or outwards (for negative value). Might help with eye strain
                                on certain stereo videos. The value is in degrees.

                                On flat projections moves the picture in the corresponding direction instead of
                                rotating it. The value is in meters.

                                Hold Alt for fine tuning.
                            "#
                            ));
                        }
                        ui.same_line();
                        if ui.button("Reset") {
                            if fdata.projection == Projection::Flat {
                                fdata.stereo_convergence_flat = 0.0;
                            } else {
                                fdata.stereo_convergence = 0.0;
                            }
                        }
                        ui.same_line();
                        ui.set_next_item_width(150.0);

                        let stereo_button = |text: &str, val: f32, pval: &mut f32| {
                            if ui.button_with_size(text, [70.0, 0.0]) {
                                *pval += val;
                            }
                        };

                        if fdata.projection == Projection::Flat {
                            let tmp_str = &mut self.tmp_str;
                            tmp_str.clear();
                            write!(tmp_str, "{:.3}m", fdata.stereo_convergence_flat).unwrap();

                            imgui::Drag::new("##stereo_adjust")
                                .range(-10.0, 10.0)
                                .speed(0.001)
                                .flags(imgui::SliderFlags::NO_INPUT)
                                .display_format(tmp_str)
                                .build(&ui, &mut fdata.stereo_convergence_flat);

                            ui.same_line_with_spacing(0.0, 25.0);
                            stereo_button("-1cm", -0.01, &mut fdata.stereo_convergence_flat);
                            ui.same_line();
                            stereo_button("+1cm", 0.01, &mut fdata.stereo_convergence_flat);
                            ui.same_line_with_spacing(0.0, 25.0);
                            stereo_button("-10cm", -0.1, &mut fdata.stereo_convergence_flat);
                            ui.same_line();
                            stereo_button("+10cm", 0.1, &mut fdata.stereo_convergence_flat);
                        } else {
                            let tmp_str = &mut self.tmp_str;
                            tmp_str.clear();
                            write!(tmp_str, "{:.3}°", fdata.stereo_convergence).unwrap();

                            imgui::Drag::new("##stereo_adjust")
                                .range(-10.0, 10.0)
                                .speed(0.01)
                                .flags(imgui::SliderFlags::NO_INPUT)
                                .display_format(tmp_str)
                                .build(&ui, &mut fdata.stereo_convergence);

                            ui.same_line_with_spacing(0.0, 25.0);
                            stereo_button("-0.1°", -0.1, &mut fdata.stereo_convergence);
                            ui.same_line();
                            stereo_button("+0.1°", 0.1, &mut fdata.stereo_convergence);
                            ui.same_line_with_spacing(0.0, 25.0);
                            stereo_button("-1°", -1.0, &mut fdata.stereo_convergence);
                            ui.same_line();
                            stereo_button("+1°", 1.0, &mut fdata.stereo_convergence);
                        }
                    }

                    // HWDEC
                    ui.align_text_to_frame_padding();
                    {
                        if ui.button(&self.hwdec) {
                            action_bin.put(Action::Command(vec![
                                "cycle-values".to_owned(),
                                "hwdec".to_owned(),
                                "no".to_owned(),
                                "auto".to_owned(),
                            ]));
                        }
                        if ui.is_item_hovered() {
                            ui.tooltip_text("Toggle hwdec mode (no/auto)");
                        }
                        let tmp_str = &mut self.tmp_str;
                        tmp_str.clear();
                        write!(tmp_str, "Hardware Decoding: {}", &self.hwdec_current).unwrap();
                        ui.same_line();
                        ui.text(tmp_str);
                    }
                }

                if ui.collapsing_header("Tracks", imgui::TreeNodeFlags::empty()) {
                    let vid = tracks.as_ref().map(|v| v.vid).unwrap_or(0);
                    let aid = tracks.as_ref().map(|v| v.aid).unwrap_or(0);
                    let sid = tracks.as_ref().map(|v| v.sid).unwrap_or(0);
                    let track_to_str = |t: &Track, str: &mut String| {
                        str.clear();
                        write!(str, "{}", t.id).unwrap();
                        *str += ":";
                        if t.lang != "" {
                            *str += " ";
                            *str += &t.lang;
                        }
                        if t.title != "" {
                            *str += " ";
                            *str += &t.title;
                        }
                        if t.codec != "" {
                            *str += " (";
                            *str += &t.codec;
                            *str += ")";
                        }
                    };
                    let mut track_list = |title: &str, idx: i64, list: Option<&[Track]>| -> Option<i64> {
                        imgui::TreeNode::new(title)
                            .flags(imgui::TreeNodeFlags::DEFAULT_OPEN)
                            .build(ui, || {
                                let mut result = None;
                                for v in list.unwrap_or(&[]) {
                                    let tmp_str = &mut self.tmp_str;
                                    track_to_str(v, tmp_str);
                                    if imgui::Selectable::new(tmp_str).selected(idx == v.id).build(ui) {
                                        result = Some(v.id);
                                    }
                                }
                                result
                            })
                            .flatten()
                    };
                    if let Some(vid) = track_list("Video", vid, tracks.as_deref().map(|v| v.video.as_slice())) {
                        action_bin.put(Action::Command(vec![
                            "set".to_owned(),
                            "vid".to_owned(),
                            format!("{}", vid),
                        ]));
                    }
                    if let Some(aid) = track_list("Audio", aid, tracks.as_deref().map(|v| v.audio.as_slice())) {
                        action_bin.put(Action::Command(vec![
                            "set".to_owned(),
                            "aid".to_owned(),
                            format!("{}", aid),
                        ]));
                    }
                    if let Some(sid) = track_list("Subtitles", sid, tracks.as_deref().map(|v| v.sub.as_slice())) {
                        action_bin.put(Action::Command(vec![
                            "set".to_owned(),
                            "sid".to_owned(),
                            format!("{}", sid),
                        ]));
                    }
                }

                if ui.collapsing_header("Settings", imgui::TreeNodeFlags::empty()) {
                    let mut ui_angle = config_syncer.get().ui_angle;
                    let mut ui_distance = config_syncer.get().ui_distance;
                    let mut ui_scale = config_syncer.get().ui_scale;
                    let mut camera_movement_speed = config_syncer.get().camera_movement_speed;
                    let mut camera_sensitivity = config_syncer.get().camera_sensitivity;
                    let mut cursor_sensitivity = config_syncer.get().cursor_sensitivity;

                    if imgui::InputFloat::new(ui, "UI Angle", &mut ui_angle).step(1.0).build() {
                        config_syncer.get_mut().ui_angle = ui_angle;
                    }

                    if imgui::InputFloat::new(ui, "UI Distance", &mut ui_distance)
                        .step(0.01)
                        .build()
                    {
                        config_syncer.get_mut().ui_distance = ui_distance;
                    }

                    if imgui::InputFloat::new(ui, "UI Scale", &mut ui_scale).step(0.01).build() {
                        config_syncer.get_mut().ui_scale = ui_scale;
                    }

                    if imgui::InputFloat::new(ui, "Camera Movement Speed", &mut camera_movement_speed)
                        .step(0.1)
                        .build()
                    {
                        config_syncer.get_mut().camera_movement_speed = camera_movement_speed;
                    }

                    if imgui::InputFloat::new(ui, "Camera Sensitivity", &mut camera_sensitivity)
                        .step(0.01)
                        .build()
                    {
                        config_syncer.get_mut().camera_sensitivity = camera_sensitivity;
                    }

                    if imgui::InputFloat::new(ui, "Cursor Sensitivity", &mut cursor_sensitivity)
                        .step(0.1)
                        .build()
                    {
                        config_syncer.get_mut().cursor_sensitivity = cursor_sensitivity;
                    }
                }

                if ui.collapsing_header("Debug", imgui::TreeNodeFlags::empty()) {
                    imgui::Drag::new("Shader Debug")
                        .speed(0.01)
                        .build(ui, &mut self.shader_debug);

                    if ui.button("Show Demo") {
                        self.show_demo = true;
                    }
                }
            });

        if self.show_demo {
            ui.show_demo_window(&mut self.show_demo);
        }
    }
}
