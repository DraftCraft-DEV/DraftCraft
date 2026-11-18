//! Workspace tooling: `cargo xtask <command>`.
//!
//! Pure Rust (std + serde_json). External tools (`cargo`, `curl`, `tar`) are
//! invoked through `std::process::Command`.

mod assets;
mod ico;
mod layers;
mod parity;
mod stats;
mod version;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  assets          check that every icon/image/font/asset is attributed in ATTRIBUTION.md
  layers          enforce the crate dependency layering (plan/architecture.md §3)
  wasm            cargo check --target wasm32-unknown-unknown for the wasm-safe crates
  test            cargo nextest run (cargo test without nextest) plus the doctests
  i18n            print the strings each interface language shows in English (never fails)
  ci [--solids]   fmt --check, clippy -D warnings, test, i18n, assets, layers, wasm (stops at first failure);
                  --solids: clippy and test with the 3D solids feature on, and the kernel's wasm check
  corpus [DIR] [--out DIR] [--keep-saved] [--same-format]
                  open -> save -> reopen every DXF/DWG of a real-file corpus (DIR, default
                  $DRAFTCRAFT_CORPUS_DIR or ../draftcraft-corpus) and write report.md + report.json
                  (default DIR/report); runs `draftcraft-cli roundtrip` in release mode
  stats [--exact] count tests and lines per crate (--exact: ask the test harness via `-- --list`)
  parity          recompute the AutoCAD menu-breadth checklist in docs/parity-checklist.md
  ico <out.ico> <in.png>...
                  pack square PNGs (<= 256 px) into a Windows .ico (see packaging/icons.sh)
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest: Vec<&str> = args.iter().skip(1).map(String::as_str).collect();
    let result = match args.first().map(String::as_str) {
        Some("assets") => assets::run(&root()),
        Some("layers") => cmd_layers(),
        Some("wasm") => cmd_wasm(),
        Some("test") => cmd_test(&[]),
        Some("i18n") => cmd_i18n(),
        Some("ci") => cmd_ci(rest.contains(&"--solids")),
        Some("corpus") => cmd_corpus(&rest),
        Some("stats") => stats::run(&root(), rest.contains(&"--exact")),
        Some("parity") => parity::run(&root()),
        Some("ico") => ico::run(&rest),
        Some("version") => version::run(&root(), &rest),
        Some("-h" | "--help" | "help") | None => {
            print!("{USAGE}");
            Ok(())
        }
        Some(other) => Err(format!("unknown command `{other}`\n\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Workspace root (parent of the xtask crate).
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask has a parent dir").to_path_buf()
}

pub fn cargo() -> Command {
    let mut c = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    c.current_dir(root());
    c
}

fn run(mut cmd: Command, what: &str) -> Result<(), String> {
    eprintln!("$ {what}");
    let status = cmd.status().map_err(|e| format!("{what}: failed to spawn: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{what}: exited with {status}")) }
}

pub fn metadata() -> Result<serde_json::Value, String> {
    let out = cargo().args(["metadata", "--format-version", "1", "--no-deps"]).output().map_err(|e| format!("cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(format!("cargo metadata failed:\n{}", String::from_utf8_lossy(&out.stderr)));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("cargo metadata: bad JSON: {e}"))
}

fn cmd_layers() -> Result<(), String> {
    let crates = layers::from_metadata(&metadata()?)?;
    println!("Dependency layering (plan/architecture.md §3)\n");
    println!("{:<28} {:<14} workspace deps", "crate", "layer");
    for c in &crates {
        let ws: Vec<String> = c
            .deps
            .iter()
            .filter(|d| d.workspace)
            .map(|d| {
                let k = match d.kind {
                    layers::DepKind::Normal => "",
                    layers::DepKind::Dev => " (dev)",
                    layers::DepKind::Build => " (build)",
                };
                format!("{}{k}", layers::short_name(&d.name))
            })
            .collect();
        println!("{:<28} {:<14} {}", c.name, layers::describe(layers::classify(&c.name)), ws.join(", "));
    }
    let violations = layers::check(&crates);
    println!();
    if violations.is_empty() {
        println!("OK: {} crates, no layering violations.", crates.len());
        Ok(())
    } else {
        println!("{} violation(s):", violations.len());
        for v in &violations {
            println!("  - {v}");
        }
        Err(format!("{} layering violation(s)", violations.len()))
    }
}

/// Workspace packages that must build for wasm32: all L0–L5 crates plus
/// the egui shell.
fn wasm_set() -> Result<Vec<String>, String> {
    let crates = layers::from_metadata(&metadata()?)?;
    Ok(crates
        .into_iter()
        .filter(|c| match layers::classify(&c.name) {
            Some(layers::Class::Layer(l)) => l <= 4 || layers::short_name(&c.name) == "ui-egui",
            Some(layers::Class::Standalone) => true,
            _ => false,
        })
        .map(|c| c.name)
        .collect())
}

fn cmd_wasm() -> Result<(), String> {
    let set = wasm_set()?;
    let mut c = cargo();
    c.args(["check", "--target", "wasm32-unknown-unknown", "--keep-going"]);
    for pkg in &set {
        c.args(["-p", pkg]);
    }
    run(c, &format!("cargo check --target wasm32-unknown-unknown --keep-going ({} crates)", set.len())).map_err(|e| format!("wasm check failed: {e}"))
}

/// The features `ci --solids` turns on: the 3D solids feature of every app (docs/solids.md).
const SOLIDS_FEATURES: &str = "draftcraft/solids,draftcraft-cli/solids";

/// The solids kernel checked for wasm32 (the web build keeps solids off, but the kernel must
/// keep building there).
fn cmd_wasm_solids() -> Result<(), String> {
    let mut c = cargo();
    c.args(["check", "--target", "wasm32-unknown-unknown", "-p", "draftcraft-solid", "--features", "kernel"]);
    run(c, "cargo check --target wasm32-unknown-unknown -p draftcraft-solid --features kernel").map_err(|e| format!("solids wasm check failed: {e}"))
}

/// Whether `cargo nextest` is installed.
fn has_nextest() -> bool {
    let mut c = cargo();
    c.args(["nextest", "--version"]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    c.status().is_ok_and(|s| s.success())
}

/// Unit and integration tests (examples are not built) with nextest, or `cargo test` when
/// nextest is missing, then the doctests, which nextest does not run.
fn cmd_test(extra: &[&str]) -> Result<(), String> {
    let more = extra.join(" ");
    let mut c = cargo();
    if has_nextest() {
        c.args(["nextest", "run", "--workspace", "--lib", "--bins", "--tests"]).args(extra);
        run(c, &format!("cargo nextest run --workspace --lib --bins --tests {more}"))?;
    } else {
        eprintln!("note: cargo-nextest is not installed (cargo install cargo-nextest --locked); running cargo test");
        c.args(["test", "--workspace", "--lib", "--bins", "--tests"]).args(extra);
        run(c, &format!("cargo test --workspace --lib --bins --tests {more}"))?;
    }
    let mut c = cargo();
    c.args(["test", "--workspace", "--doc"]).args(extra);
    run(c, &format!("cargo test --workspace --doc {more}"))
}

/// The translation gap report (`draftcraft-ui-egui`'s ignored `i18n::tests::translation_gaps`).
/// A missing translation shows in English and never fails a build, so neither does this step.
fn cmd_i18n() -> Result<(), String> {
    let mut c = cargo();
    // `--workspace` builds the same test binaries as the test step; `-p` alone would rebuild them.
    c.args(["test", "--workspace", "--lib", "-q", "--", "--ignored", "--exact", "i18n::tests::translation_gaps", "--nocapture"]);
    if let Err(e) = run(c, "cargo test --workspace --lib -q -- --ignored --exact i18n::tests::translation_gaps --nocapture") {
        eprintln!("note: the translation gap report did not run: {e}");
    }
    Ok(())
}

fn cmd_ci(solids: bool) -> Result<(), String> {
    type Step = (&'static str, Box<dyn Fn() -> Result<(), String>>);
    let extra: Vec<&'static str> = if solids { vec!["--features", SOLIDS_FEATURES] } else { Vec::new() };
    let (clippy_extra, test_extra) = (extra.clone(), extra);
    let mut steps: Vec<Step> = vec![
        (
            "fmt",
            Box::new(|| {
                let mut c = cargo();
                c.args(["fmt", "--all", "--", "--check"]);
                run(c, "cargo fmt --all -- --check")
            }),
        ),
        (
            "clippy",
            Box::new(move || {
                let mut c = cargo();
                c.args(["clippy", "--workspace", "--all-targets", "--keep-going"]).args(&clippy_extra).args(["--", "-D", "warnings"]);
                run(c, &format!("cargo clippy --workspace --all-targets --keep-going {} -- -D warnings", clippy_extra.join(" ")))
            }),
        ),
        ("test", Box::new(move || cmd_test(&test_extra))),
        ("i18n", Box::new(cmd_i18n)),
        ("assets", Box::new(|| assets::run(&root()))),
        ("layers", Box::new(cmd_layers)),
        ("wasm", Box::new(cmd_wasm)),
    ];
    if solids {
        steps.push(("wasm-solids", Box::new(cmd_wasm_solids)));
    }
    let mut done = Vec::new();
    for (name, f) in &steps {
        eprintln!("\n=== ci: {name} ===");
        if let Err(e) = f() {
            println!("\nCI summary:");
            for d in &done {
                println!("  ok    {d}");
            }
            println!("  FAIL  {name}: {e}");
            for (n, _) in steps.iter().skip(done.len() + 1) {
                println!("  skip  {n}");
            }
            return Err(format!("ci failed at `{name}`"));
        }
        done.push(*name);
    }
    println!("\nCI summary: all {} steps passed ({})", done.len(), done.join(", "));
    Ok(())
}

/// `corpus [DIR] [options]`: the round-trip report over a real-file corpus (gaps FF1, FF5). The
/// corpus lives outside this repo (any folder of DWG/DXF files); its files are never committed here.
fn cmd_corpus(args: &[&str]) -> Result<(), String> {
    // `draftcraft-cli` runs from the repository root: relative paths are made absolute against the
    // folder `cargo xtask` was run from first, so they mean what the user typed.
    let here = std::env::current_dir().map_err(|e| format!("current folder: {e}"))?;
    let mut pass: Vec<String> = Vec::new();
    let mut dir: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match *a {
            "--out" => {
                let v = it.next().ok_or_else(|| format!("{a} needs a value"))?;
                pass.extend([a.to_string(), here.join(v).display().to_string()]);
            }
            "--tolerance" => {
                let v = it.next().ok_or_else(|| format!("{a} needs a value"))?;
                pass.extend([a.to_string(), v.to_string()]);
            }
            s if s.starts_with("--out=") => {
                let v = s.trim_start_matches("--out=");
                pass.extend(["--out".to_string(), here.join(v).display().to_string()]);
            }
            s if s.starts_with("--") => pass.push(s.to_string()),
            s if dir.is_none() => dir = Some(s.to_string()),
            _ => return Err("corpus takes one DIR".into()),
        }
    }
    let dir = dir.or_else(|| std::env::var("DRAFTCRAFT_CORPUS_DIR").ok()).unwrap_or_else(|| {
        root().parent().map(|p| p.join("draftcraft-corpus").display().to_string()).unwrap_or_else(|| "../draftcraft-corpus".into())
    });
    let dir = here.join(&dir);
    if !dir.is_dir() {
        return Err(format!(
            "no corpus at `{}`: put a folder of DWG/DXF files next to this repo, set DRAFTCRAFT_CORPUS_DIR or pass DIR \
             (never commit corpus files to this repo)",
            dir.display()
        ));
    }
    let dir = dir.display().to_string();
    let mut c = cargo();
    c.args(["run", "--release", "-q", "-p", "draftcraft-cli", "--", "roundtrip", &dir]).args(&pass);
    run(c, &format!("draftcraft-cli roundtrip {dir} {}", pass.join(" ")))
}
