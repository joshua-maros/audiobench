use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

fn main() {
    // Can't use env! because it isn't defined when the build script is first compiled.
    let output_path = Path::new(&std::env::var("OUT_DIR").unwrap()).join("Factory.ablib");
    let output_file = fs::File::create(output_path).unwrap();
    let mut zip_writer = zip::ZipWriter::new(output_file);
    let options =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let input_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("factory_library");
    println!("cargo:rerun-if-changed={:?}", input_path.as_os_str());
    // https://github.com/mvdnes/zip-rs/blob/master/examples/write_dir.rs
    for entry in walkdir::WalkDir::new(input_path.clone()).into_iter() {
        let entry = entry.unwrap();
        let path = entry.path();
        let zip_key = path
            .strip_prefix(input_path.clone())
            .unwrap()
            .to_string_lossy()
            .to_string();
        if path.is_file() {
            zip_writer.start_file(&zip_key, options.clone()).unwrap();
            let mut f = fs::File::open(path).unwrap();
            if zip_key == "library_info.yaml" {
                let engine_version = std::env::var("CARGO_PKG_VERSION").unwrap();
                let mut file_contents = String::new();
                f.read_to_string(&mut file_contents).unwrap();
                file_contents = file_contents.replace("$ENGINE_VERSION", &engine_version);
                zip_writer.write_all(file_contents.as_bytes()).unwrap();
            } else {
                std::io::copy(&mut f, &mut zip_writer).unwrap();
            }
        } else if zip_key.len() > 0 {
            zip_writer.add_directory(&zip_key, options.clone()).unwrap();
        }
    }
    zip_writer.finish().unwrap();
}
