use regex::Regex;

use std::io::prelude::*;
use std::fs::File;
use std::env;

#[tokio::main]
async fn main() {
    let version = env::var_os("CARGO_PKG_VERSION").unwrap();
    
    let response = reqwest::get(format!("https://raw.githubusercontent.com/less/less.js/v{}/dist/less.min.js", version.to_string_lossy()).as_str()).await.expect(format!("Failed to download Less ({})", version.to_string_lossy()).as_str());
    let content =  response.text().await.expect(format!("Failed to download Less ({})", version.to_string_lossy()).as_str());

    let mut file = File::create("src/less.js").expect(format!("Failed to download Less ({})", version.to_string_lossy()).as_str());
    
    let content = Regex::new(r"\b(?:window|document)\b").expect(format!("Failed to download Less ({})", version.to_string_lossy()).as_str()).replace_all(&content, "__v8_dummy_object__");

    file.write_all(content.as_bytes()).expect(format!("Failed to download Less ({})", version.to_string_lossy()).as_str());
    
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");
    println!("cargo:rerun-if-changed=build.rs");
}