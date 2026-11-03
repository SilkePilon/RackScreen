//! Copy the built frontend (`<workspace>/web/dist`) into `$OUT_DIR/ui` so
//! `include_dir!` can embed it. Without a build, a placeholder page says how
//! to make one.

use std::fs;
use std::path::{Path, PathBuf};

const PLACEHOLDER: &str = "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><title>RackScreen</title>\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<style>body{font:16px/1.5 system-ui,sans-serif;background:#0b0b0c;color:#ededef;margin:0;display:grid;place-items:center;min-height:100vh}\
main{max-width:36rem;padding:2rem}code{background:#1c1c21;padding:.15em .4em;border-radius:.3em}</style></head>\
<body><main><h1>RackScreen</h1><p>The web UI is not built into this binary. On a checkout run</p>\
<p><code>npm --prefix web ci &amp;&amp; npm --prefix web run build</code></p>\
<p>and rebuild <code>rackscreen</code>. The HTTP API under <code>/api</code> works regardless; \
try <a href=\"/api/status\">/api/status</a>.</p></main></body></html>\n";

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let dist = manifest.join("../../web/dist");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("out dir")).join("ui");
    println!("cargo:rerun-if-changed={}", dist.display());
    println!("cargo:rerun-if-changed=build.rs");
    if out.exists() {
        fs::remove_dir_all(&out).expect("clear old ui dir");
    }
    if dist.join("index.html").is_file() {
        copy_dir(&dist, &out).expect("copy web/dist");
        println!("cargo:rustc-cfg=ui_built");
    } else {
        fs::create_dir_all(&out).expect("create ui dir");
        fs::write(out.join("index.html"), PLACEHOLDER).expect("write placeholder");
    }
    println!("cargo:rustc-check-cfg=cfg(ui_built)");
}
