fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/paint-10.rc");
    println!("cargo:rerun-if-changed=assets/paint-10.ico");

    // Build scripts run on the host. Inspect the target explicitly so this
    // also embeds the icon when cross-compiling Windows from another OS.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    embed_resource::compile_for(
        "assets/paint-10.rc",
        ["paint-10"],
        embed_resource::ParamsIncludeDirs(["assets"]),
    )
    .manifest_required()
    .expect("The Windows build requires a resource compiler to embed the Paint 10 icon");
}
