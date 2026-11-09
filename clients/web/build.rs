use std::env;
use std::process::Command;

fn main() {
    println!("cargo::rerun-if-changed=js");
    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("wasm32") {
        return;
    }

    let built = Command::new(if cfg!(windows) { "node.exe" } else { "node" })
        .arg("js/build.mjs")
        .status()
        .expect("node should run js/build.mjs, which bundles the editor's JavaScript");
    assert!(built.success(), "js/build.mjs failed");
}
