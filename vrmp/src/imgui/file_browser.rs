use crate::action::{Action, ActionBin};
use crate::config::ConfigSyncer;
use crate::filedb::{load_file_hash, FileDB};
use crate::imgui::font_awesome as fa;
use std::ffi::OsStr;
use std::fmt::Write;
use std::{ffi::OsString, fs::Metadata, path::PathBuf};

use super::util::hex;

fn is_video_extension(ext: Option<&OsStr>) -> bool {
    if let Some(ext) = ext {
        ext.eq_ignore_ascii_case("avi")
            || ext.eq_ignore_ascii_case("flv")
            || ext.eq_ignore_ascii_case("m4p")
            || ext.eq_ignore_ascii_case("m4v")
            || ext.eq_ignore_ascii_case("mkv")
            || ext.eq_ignore_ascii_case("mov")
            || ext.eq_ignore_ascii_case("mp2")
            || ext.eq_ignore_ascii_case("mp4")
            || ext.eq_ignore_ascii_case("mpe")
            || ext.eq_ignore_ascii_case("mpeg")
            || ext.eq_ignore_ascii_case("mpg")
            || ext.eq_ignore_ascii_case("mpv")
            || ext.eq_ignore_ascii_case("ogg")
            || ext.eq_ignore_ascii_case("qt")
            || ext.eq_ignore_ascii_case("swf")
            || ext.eq_ignore_ascii_case("webm")
            || ext.eq_ignore_ascii_case("wmv")
    } else {
        false
    }
}

pub struct ImguiFileBrowser {
    current_path: PathBuf,
    contents: Vec<(OsString, Metadata, Option<(u64, u64)>)>,
    tmp_str: String,
    tmp_path: PathBuf,
}

impl ImguiFileBrowser {
    pub fn new(fdb: &mut FileDB) -> ImguiFileBrowser {
        let mut res = ImguiFileBrowser {
            current_path: std::env::current_dir().unwrap(),
            contents: Vec::new(),
            tmp_str: String::new(),
            tmp_path: PathBuf::new(),
        };
        res.rebuild(fdb);
        res
    }

    fn rebuild(&mut self, fdb: &mut FileDB) {
        self.contents.clear();
        if let Ok(rd) = std::fs::read_dir(&self.current_path) {
            for f in rd {
                if let Ok(e) = f {
                    if let Ok(md) = e.metadata() {
                        let file_name = e.file_name();
                        let tmp_path = &mut self.tmp_path;
                        tmp_path.clone_from(&self.current_path);
                        tmp_path.push(&file_name);
                        let is_video = is_video_extension(tmp_path.extension());
                        let key = if is_video {
                            load_file_hash(&tmp_path).map(|hash| (md.len(), hash))
                        } else {
                            None
                        };
                        self.contents.push((file_name, md, key));
                        if let Some(key) = key {
                            if let Err(e) = fdb.preload_file(key.0, key.1) {
                                log::error!("failed preloading file: {}", e);
                            }
                        }
                    }
                }
            }
        }

        self.contents.sort_by(|a, b| {
            if a.1.is_dir() != b.1.is_dir() {
                return if a.1.is_dir() {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                };
            } else {
                a.0.cmp(&b.0)
            }
        });
    }

    pub fn render(
        &mut self,
        action_bin: &mut ActionBin,
        config_syncer: &mut ConfigSyncer,
        fdb: &mut FileDB,
        ui: &imgui::Ui,
        position: [f32; 2],
        size: [f32; 2],
    ) {
        let window = imgui::Window::new("File Browser");
        window
            .flags(imgui::WindowFlags::NO_RESIZE | imgui::WindowFlags::NO_TITLE_BAR)
            .position(position, imgui::Condition::FirstUseEver)
            .size(size, imgui::Condition::FirstUseEver)
            .build(ui, || {
                // settings
                {
                    let mut show_video_files_only = config_syncer.get().show_video_files_only;
                    let mut show_hidden_files = config_syncer.get().show_hidden_files;
                    if ui.checkbox("Video Files Only", &mut show_video_files_only) {
                        config_syncer.get_mut().show_video_files_only = show_video_files_only;
                    }
                    ui.same_line();
                    if ui.checkbox("Hidden Files", &mut show_hidden_files) {
                        config_syncer.get_mut().show_hidden_files = show_hidden_files;
                    }
                }
                // current path line
                {
                    let tmp_str = &mut self.tmp_str;

                    let _token1 = ui.push_style_var(imgui::StyleVar::ItemSpacing([0.0, 4.0]));
                    let _token2 = ui.push_style_var(imgui::StyleVar::FramePadding([0.0, 0.0]));

                    let mut clicked_index = None;
                    for (i, comp) in self.current_path.iter().enumerate() {
                        let s = comp.to_string_lossy();
                        tmp_str.clear();
                        if i > 1 {
                            write!(tmp_str, "/{}", s).unwrap();
                        } else {
                            write!(tmp_str, "{}", s).unwrap();
                        }
                        if i != 0 {
                            ui.same_line();
                        }
                        if ui.small_button(&tmp_str) {
                            clicked_index = Some(i);
                        }
                    }

                    // event processing
                    if let Some(i) = clicked_index {
                        let mut num_elements = self.current_path.iter().count();
                        while num_elements > i + 1 {
                            self.current_path.pop();
                            num_elements -= 1;
                        }
                        self.rebuild(fdb);
                    }
                }

                // favorites
                {
                    ui.same_line_with_pos(ui.window_content_region_width() - 65.0);
                    let favidx = {
                        let cfg = config_syncer.get();
                        cfg.favorite_directories.iter().position(|pp| pp == &self.current_path)
                    };
                    {
                        {
                            let _token =
                                favidx.map(|_| ui.push_style_color(imgui::StyleColor::Text, [0.98, 0.831, 0.004, 1.0]));
                            if ui.small_button(fa::BOOKMARK) {
                                let cfg_mut = config_syncer.get_mut();
                                if let Some(idx) = favidx {
                                    cfg_mut.favorite_directories.remove(idx);
                                } else {
                                    let p = self.current_path.clone();
                                    cfg_mut.favorite_directories.push(p);
                                }
                            }
                        }
                        if ui.is_item_hovered() {
                            if favidx.is_some() {
                                ui.tooltip_text("Remove from favorites");
                            } else {
                                ui.tooltip_text("Add to favorites");
                            }
                        }
                    }
                    {
                        ui.same_line();
                        let _token = ui.push_style_var(imgui::StyleVar::FramePadding([0.0, 0.0]));
                        let cfg = config_syncer.get();
                        imgui::ComboBox::new("##favorites")
                            .flags(imgui::ComboBoxFlags::NO_PREVIEW | imgui::ComboBoxFlags::POPUP_ALIGN_LEFT)
                            .build(ui, || {
                                for dir in &cfg.favorite_directories {
                                    if imgui::Selectable::new(dir.to_string_lossy()).build(ui) {
                                        self.current_path.clone_from(dir);
                                        self.rebuild(fdb);
                                    }
                                }
                            });
                    }
                }

                // current path children entries
                if let Some(_w) = imgui::ChildWindow::new("dir-entries")
                    .size([0.0, 0.0])
                    .border(true)
                    .always_horizontal_scrollbar(true)
                    .begin(ui)
                {
                    // ".." line for "go to parent" action
                    if self.current_path.parent().is_some() {
                        let tmp_str = &mut self.tmp_str;
                        tmp_str.clear();
                        write!(tmp_str, "{}  ..", fa::FOLDER).unwrap();

                        if imgui::Selectable::new(tmp_str).build(ui) {
                            self.current_path.pop();
                            self.rebuild(fdb);
                        }
                    }

                    let mut clicked_dir = None;
                    let mut clicked_file = None;
                    let show_hidden_files = config_syncer.get().show_hidden_files;
                    let show_video_files_only = config_syncer.get().show_video_files_only;

                    // render ui for entries
                    for c in &self.contents {
                        let is_seen = c.2.and_then(|k| fdb.get_file(k).map(|_| true)).unwrap_or(false);
                        let name = c.0.to_string_lossy();
                        {
                            let tmp_str = &mut self.tmp_str;
                            tmp_str.clear();
                        }

                        if name.bytes().next() == Some(b'.') && !show_hidden_files {
                            continue;
                        }
                        let is_dir = c.1.is_dir();
                        let clicked = {
                            let tmp_str = &mut self.tmp_str;
                            tmp_str.clear();
                            let _token = if is_dir {
                                write!(tmp_str, "{}  {}", fa::FOLDER, name).unwrap();
                                None
                            } else {
                                let p: &std::path::Path = c.0.as_ref();
                                let is_video = is_video_extension(p.extension());
                                if show_video_files_only && !is_video {
                                    continue;
                                }
                                let icon = cond!(is_video, fa::FILE_VIDEO, fa::FILE);
                                write!(tmp_str, "{}  ", icon).unwrap();
                                if is_seen {
                                    write!(tmp_str, "{} ", fa::EYE).unwrap();
                                }
                                write!(tmp_str, "{}", name).unwrap();
                                is_video.then(|| {
                                    ui.push_style_color(
                                        imgui::StyleColor::Text,
                                        cond!(is_seen, hex("#f7fcc6"), hex("#d7ffd8")),
                                    )
                                })
                            };
                            imgui::Selectable::new(tmp_str).build(ui)
                        };
                        if clicked {
                            if is_dir {
                                clicked_dir = Some(c.0.clone());
                            } else {
                                clicked_file = Some(c.0.clone());
                            }
                        }
                    }

                    // event processing
                    if let Some(clicked_dir) = clicked_dir {
                        self.current_path.push(clicked_dir);
                        self.rebuild(fdb);
                    } else if let Some(clicked_file) = clicked_file {
                        let mut p = self.current_path.clone();
                        p.push(clicked_file);
                        action_bin.put(Action::Command(vec![
                            "loadfile".to_owned(),
                            p.to_string_lossy().to_string(),
                        ]));
                    }
                }
            });
    }
}
