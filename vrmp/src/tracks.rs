use libmpv::Node;

#[derive(Debug)]
pub struct Track {
    pub id: i64,
    pub title: String,
    pub codec: String,
    pub lang: String,
}

#[derive(Default)]
pub struct Tracks {
    pub vid: i64,
    pub aid: i64,
    pub sid: i64,

    pub video: Vec<Track>,
    pub audio: Vec<Track>,
    pub sub: Vec<Track>,
}

impl Tracks {
    pub fn parse(n: &Node) -> Tracks {
        let list = match n.as_array() {
            Some(v) => v,
            None => return Tracks::default(),
        };
        let mut video = Vec::new();
        let mut audio = Vec::new();
        let mut sub = Vec::new();

        for n in list {
            let m = match n {
                Node::Map(v) => v,
                _ => {
                    continue;
                }
            };
            let id = m.get("id").and_then(|v| v.as_i64());
            let typ = m.get("type").and_then(|v| v.as_string());
            if let (Some(&id), Some(typ)) = (id, typ) {
                let get_str =
                    |key: &str| -> &str { m.get(key).and_then(|v| v.as_string()).map(|v| v.as_str()).unwrap_or("") };
                let lang = get_str("lang");
                let title = get_str("title");
                let codec = get_str("codec");
                let track = Track {
                    id,
                    title: title.to_owned(),
                    codec: codec.to_owned(),
                    lang: lang.to_owned(),
                };

                match typ.as_str() {
                    "video" => {
                        video.push(track);
                    }
                    "audio" => {
                        audio.push(track);
                    }
                    "sub" => {
                        sub.push(track);
                    }
                    _ => {}
                }
            }
        }
        log::info!("{:#?}", video);
        log::info!("{:#?}", audio);
        log::info!("{:#?}", sub);
        Tracks {
            vid: 0,
            sid: 0,
            aid: 0,
            video,
            audio,
            sub,
        }
    }
}
