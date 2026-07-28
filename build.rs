//! Build script: embeds the Windows version resource (product metadata + icon) into
//! the executable.
//!
//! Executables without version metadata and an icon look like anonymous binaries to
//! antivirus heuristics and get flagged as false positives far more often; the
//! resource also gives the exe its file properties and Explorer icon.

fn main() {
    // Only meaningful when compiling *for* Windows (the build script itself always
    // runs on the host).
    if std::env::var_os("CARGO_CFG_WINDOWS").is_none() {
        return;
    }

    println!("cargo:rerun-if-changed=assets/icon.ico");

    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("assets/icon.ico");
    resource.set("ProductName", "Jotphant");
    resource.set(
        "FileDescription",
        "Jotphant — Pomodoro task board with a Markdown notebook",
    );
    resource.set("CompanyName", "Halil Coşgun");
    resource.set("LegalCopyright", "© 2026 Halil Coşgun — MIT License");
    resource.set("OriginalFilename", "Jotphant.exe");
    // FileVersion / ProductVersion default to CARGO_PKG_VERSION automatically.
    resource
        .compile()
        .expect("compiling the Windows version resource (requires rc.exe from MSVC)");
}
