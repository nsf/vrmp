fn main() {
    pkg_config::Config::new()
        .atleast_version("1.16.8")
        .probe("openvr")
        .unwrap();
}
