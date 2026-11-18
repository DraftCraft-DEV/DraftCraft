//! Puts the installer into `draftcraft-setup.exe`: `DRAFTCRAFT_SETUP_MSI` names the MSI that
//! `packaging/windows/package.ps1` has just built. Without it (an ordinary `cargo build`) the setup
//! carries no installer and says so when run. On Windows it also embeds the icon and version info.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=DRAFTCRAFT_SETUP_MSI");
    println!("cargo:rerun-if-env-changed=DRAFTCRAFT_REQUIRE_WINRES");
    println!("cargo:rerun-if-changed=../../assets/app-icon/draftcraft.ico");
    let Some(out) = std::env::var_os("OUT_DIR").map(std::path::PathBuf::from) else {
        eprintln!("cargo didn't set OUT_DIR");
        std::process::exit(1);
    };
    let dest = out.join("draftcraft.msi");
    let copied = match std::env::var_os("DRAFTCRAFT_SETUP_MSI") {
        Some(msi) => {
            println!("cargo:rerun-if-changed={}", std::path::Path::new(&msi).display());
            std::fs::copy(&msi, &dest).map(|_| ())
        }
        None => std::fs::write(&dest, b""),
    };
    if let Err(e) = copied {
        eprintln!("couldn't put the installer into the setup: {e}");
        std::process::exit(1);
    }

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    if std::path::Path::new("../../assets/app-icon/draftcraft.ico").exists() {
        res.set_icon("../../assets/app-icon/draftcraft.ico");
    }
    res.set("ProductName", "DraftCraft")
        .set("FileDescription", "DraftCraft Setup")
        .set("CompanyName", "Skryx Dev")
        .set("LegalCopyright", "Copyright (c) 2026 Skryx Dev and WithShalom Studios. MIT OR Apache-2.0.")
        .set("OriginalFilename", "draftcraft-setup.exe")
        .set("InternalName", "draftcraft-setup");
    if let Err(e) = res.compile() {
        if std::env::var_os("DRAFTCRAFT_REQUIRE_WINRES").is_some() {
            eprintln!("embedding Windows resources failed: {e}");
            std::process::exit(1);
        }
        println!("cargo:warning=draftcraft-setup.exe built without icon/version resources: {e}");
    }
}
