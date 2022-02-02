use std::{collections::HashMap, path::Path};

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::enums::{AspectRatio, Mode, Projection};
use rusqlite::OptionalExtension;
use std::io::Read;

pub fn load_file_size_and_hash<P>(path: P) -> Option<(u64, u64)>
where
    P: AsRef<Path>,
{
    match std::fs::File::open(path.as_ref()) {
        Ok(mut f) => match f.metadata() {
            Ok(md) => {
                let size = md.len();
                let mut data = vec![0u8; 128 * 1024];
                if let Ok(hash_size) = f.read(&mut data) {
                    data.resize(hash_size, 0u8);
                    let hash = fxhash::hash64(&data);
                    return Some((size, hash));
                }
            }
            Err(e) => log::error!("failed reading file metadata: {}", e),
        },
        Err(e) => log::error!("failed opening file: {}", e),
    };
    None
}

pub fn load_file_hash<P>(path: P) -> Option<u64>
where
    P: AsRef<Path>,
{
    match std::fs::File::open(path.as_ref()) {
        Ok(mut f) => {
            let mut data = vec![0u8; 128 * 1024];
            if let Ok(hash_size) = f.read(&mut data) {
                data.resize(hash_size, 0u8);
                let hash = fxhash::hash64(&data);
                return Some(hash);
            }
        }
        Err(e) => log::error!("failed opening file: {}", e),
    };
    None
}

// A database with per file info, stored on disk via SQL, but also with manual in-memory cache.
// On disk we store it in a sqlite table:
// [file size] [first 128kb file hash] [data]

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct FileData {
    // Which portions of the file you saw? Each bit corresponds to 1/100th of a file (1 percent if you wish)
    // This feature is enabled for files which are longer than 500 seconds. Technically I could enable it for smaller
    // files, but what's the point. Maybe I will figure out a way to calculate it reliably later. Right now it's
    // recorded when player receives position update, it rounds the position to the closest chunk (out of 100 chunks)
    // and writes the bit. With 500 second requirement you have 5 seconds (and 5 updates) to actually hit the bit.
    //
    // Also I could use u128, but decided not to. Hard to say why. It's not like I plan to port things to wasm, where
    // u128 are not supported. Just don't feel comfortable using u128. Also "ron" serde encoder/decoder supports u128,
    // but never the less.
    pub seen0: u64,
    pub seen1: u64,

    pub projection: Projection,
    pub mode: Mode,
    pub stereo_convergence: f32,
    pub aspect_ratio: AspectRatio,

    #[serde(default = "default_stereo_convergence_flat")]
    pub stereo_convergence_flat: f32,

    #[serde(default = "default_flat_distance")]
    pub flat_distance: f32,

    #[serde(default = "default_flat_scale")]
    pub flat_scale: f32,
}

fn default_stereo_convergence_flat() -> f32 {
    0.0
}

fn default_flat_distance() -> f32 {
    5.0
}

fn default_flat_scale() -> f32 {
    4.0
}

impl FileData {
    pub fn mark_as_seen(&mut self, percentage: f64) {
        let mut p = ((percentage * 1.28).floor() as u8).clamp(0, 127);
        if p >= 64 {
            p -= 64;
            let bit = 1u64 << p;
            self.seen1 |= bit;
        } else {
            let bit = 1u64 << p;
            self.seen0 |= bit;
        }
    }

    pub fn flip_eyes(&mut self) {
        self.mode = match self.mode {
            Mode::TopBottom => Mode::BottomTop,
            Mode::BottomTop => Mode::TopBottom,
            Mode::LeftRight => Mode::RightLeft,
            Mode::RightLeft => Mode::LeftRight,
            _ => self.mode,
        };
    }
}

pub struct CachedFileData {
    // if file was saved to DB, this is how data looked
    saved_data: Option<FileData>,
    dirty: bool,
    size: u64,
    hash: u64,
    // data as it is now
    pub data: FileData,
}

impl CachedFileData {
    pub fn new_dirty(key: (u64, u64), data: FileData) -> CachedFileData {
        CachedFileData {
            dirty: true,
            ..CachedFileData::new(key, data)
        }
    }
    pub fn new(key: (u64, u64), data: FileData) -> CachedFileData {
        CachedFileData {
            size: key.0,
            hash: key.1,
            saved_data: Some(data.clone()),
            data,
            dirty: false,
        }
    }
}

fn load_sqlite() -> Result<rusqlite::Connection, anyhow::Error> {
    let dirs = xdg::BaseDirectories::with_prefix("vrmp")?;
    let conn = if let Some(sqlfile) = dirs.find_data_file("files.sqlite") {
        let conn = rusqlite::Connection::open(sqlfile)?;
        conn
    } else {
        let sqlfile = dirs.place_data_file("files.sqlite")?;
        let conn = rusqlite::Connection::open(sqlfile)?;
        conn
    };

    conn.execute(
        r#"
            CREATE TABLE IF NOT EXISTS files (
                size INT NOT NULL,
                hash BLOB NOT NULL,
                data BLOB NOT NULL,
                PRIMARY KEY (size, hash)
            );
        "#,
        [],
    )?;

    Ok(conn)
}

pub struct FileDB {
    // files loaded/written from/to DB
    pub local_file_cache: HashMap<(u64, u64), CachedFileData>,
    conn: Option<rusqlite::Connection>,
}

impl FileDB {
    pub fn load() -> FileDB {
        let conn = match load_sqlite() {
            Ok(conn) => Some(conn),
            Err(e) => {
                log::error!("failed opening sqlite db: {}", e);
                None
            }
        };
        FileDB {
            local_file_cache: HashMap::new(),
            conn,
        }
    }

    pub fn preload_file(&mut self, size: u64, hash: u64) -> Result<(), anyhow::Error> {
        if let Some(conn) = self.conn.as_ref() {
            let mut select_stmt = conn.prepare_cached("SELECT data FROM files WHERE size = ? AND hash = ?")?;
            let data = select_stmt
                .query_row(params![size, bytemuck::bytes_of(&hash)], |row| {
                    let data: Vec<u8> = row.get(0)?;
                    Ok(data)
                })
                .optional()?;
            if let Some(data) = data {
                let fdata: FileData = ron::from_str(&String::from_utf8(data)?)?;
                self.local_file_cache
                    .insert((size, hash), CachedFileData::new((size, hash), fdata));
            }
        }
        Ok(())
    }

    pub fn get_file_mut(&mut self, key: (u64, u64)) -> &mut FileData {
        if !self.local_file_cache.contains_key(&key) {
            self.local_file_cache.insert(
                key.clone(),
                CachedFileData::new_dirty(
                    key,
                    FileData {
                        mode: Mode::Mono,
                        projection: Projection::Flat,
                        seen0: 0,
                        seen1: 0,
                        stereo_convergence: 0.0,
                        aspect_ratio: AspectRatio::One,
                        stereo_convergence_flat: default_stereo_convergence_flat(),
                        flat_distance: default_flat_distance(),
                        flat_scale: default_flat_scale(),
                    },
                ),
            );
        }
        let v = self.local_file_cache.get_mut(&key).unwrap();
        v.dirty = true;
        &mut v.data
    }

    pub fn get_file(&mut self, key: (u64, u64)) -> Option<&FileData> {
        self.local_file_cache.get(&key).map(|v| &v.data)
    }

    pub fn save_to_disk_maybe(&mut self) {
        let conn = match &self.conn {
            Some(conn) => conn,
            None => return,
        };
        let mut select_stmt = match conn.prepare_cached(
            r#"
                INSERT INTO files VALUES (?, ?, ?)
                ON CONFLICT (size, hash) DO UPDATE SET
                    data = ?
            "#,
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                log::error!("failed preparing insert stmt: {}", e);
                return;
            }
        };
        for v in self.local_file_cache.values_mut() {
            if !v.dirty {
                continue;
            }

            v.dirty = false;
            if Some(v.data.clone()) != v.saved_data {
                v.saved_data = Some(v.data.clone());
                match ron::to_string(&v.data).map(|v| Vec::from(v)) {
                    Ok(data) => {
                        if let Err(e) = select_stmt.execute(params![v.size, bytemuck::bytes_of(&v.hash), &data, &data])
                        {
                            log::error!("failed saving file: {}", e);
                        }
                    }
                    Err(e) => {
                        log::error!("failed marshaling file data into ron: {}", e);
                    }
                }
            }
        }
    }
}
