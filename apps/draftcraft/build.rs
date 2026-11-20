//! Windows only: embed the app icon and version info into `draftcraft.exe`, and give its main
//! thread the stack Linux and macOS give theirs. Elsewhere a no-op.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/app-icon/draftcraft.ico");
    println!("cargo:rerun-if-env-changed=DRAFTCRAFT_REQUIRE_WINRES");
    println!("cargo:rerun-if-env-changed=DRAFTCRAFT_BUILD_SHA");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // The main thread's stack, 1 MB on Windows by default, as large as on Linux and macOS (8 MB):
    // the TraceDraft tab's editor draws on it, and a drawing of a few hundred nested groups needs
    // more than 1 MB there.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bin=draftcraft=/STACK:8388608");
    } else {
        println!("cargo:rustc-link-arg-bin=draftcraft=-Wl,--stack,8388608");
    }
    let mut res = winresource::WindowsResource::new();
    if std::path::Path::new("../../assets/app-icon/draftcraft.ico").exists() {
        res.set_icon("../../assets/app-icon/draftcraft.ico");
    }
    res.set("ProductName", "DraftCraft")
        .set("FileDescription", "DraftCraft computer-aided design")
        .set("CompanyName", "Skryx Dev")
        .set("LegalCopyright", "Copyright (c) 2026 Skryx Dev and WithShalom Studios. MIT OR Apache-2.0.")
        .set("OriginalFilename", "draftcraft.exe")
        .set("InternalName", "draftcraft");
    if let Err(e) = res.compile() {
        if std::env::var_os("DRAFTCRAFT_REQUIRE_WINRES").is_some() {
            eprintln!("embedding Windows resources failed: {e}");
            std::process::exit(1);
        }
        println!("cargo:warning=draftcraft.exe built without icon/version resources: {e}");
    }
}
