use std::{env, fs, path::PathBuf};

fn main() {
    let path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("prayer-api/openapi/prayer-v1.json"));
    let bytes = serde_json::to_vec_pretty(&prayer_api::openapi_v1()).expect("serialize OpenAPI");
    fs::write(&path, [bytes, b"\n".to_vec()].concat())
        .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
}
