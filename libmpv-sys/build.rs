fn main() {
    pkg_config::Config::new()
        .atleast_version("1.109.0")
        .probe("mpv")
        .unwrap();
}
