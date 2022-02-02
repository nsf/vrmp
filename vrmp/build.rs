use std::{env, error::Error, fs};

fn generate_shaders() -> std::result::Result<(), Box<dyn Error>> {
    let tera = tera::Tera::new("src/shaders/**/*")?;
    println!("cargo:rerun-if-changed=src/shaders/");
    let context = tera::Context::new();
    // TODO: add things to context

    let output_path = env::var("OUT_DIR")?;
    fs::create_dir_all(format!("{}/shaders/", output_path))?;
    for dir_entry in fs::read_dir("src/shaders")? {
        let dir_entry = dir_entry?;
        let file = dir_entry.file_name();
        let file_name = file.to_str().unwrap();
        let result = tera.render(file_name, &context)?;
        // TODO: validate shaders using naga at build time
        fs::write(format!("{}/shaders/{}", output_path, file_name), result)?;
        println!("cargo:rerun-if-changed=src/shaders/{}", file_name);
    }
    Ok(())
}

fn main() {
    if let Err(err) = generate_shaders() {
        panic!("Unable to generate shaders\n{}", err);
    }
}
