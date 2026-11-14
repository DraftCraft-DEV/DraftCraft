//! `cargo xtask version [set X.Y.Z[-pre]]`: the workspace version, the single source of truth.
//!
//! The version lives in `[workspace.package] version` in the root `Cargo.toml`; every crate
//! inherits it with `version.workspace = true`, and the packaging scripts read it from here.

use std::path::Path;

/// The `version = "…"` value inside `[workspace.package]`.
pub fn read(manifest: &str) -> Result<String, String> {
    let (_, line) = find_line(manifest)?;
    let v = line.split_once('=').map(|(_, v)| v.trim().trim_matches('"').to_string()).unwrap_or_default();
    if v.is_empty() {
        return Err("empty [workspace.package] version".into());
    }
    Ok(v)
}

/// `manifest` with the workspace version replaced; everything else is byte-for-byte unchanged.
pub fn replace(manifest: &str, new: &str) -> Result<String, String> {
    validate(new)?;
    let (idx, _) = find_line(manifest)?;
    let mut out = String::with_capacity(manifest.len() + 8);
    for (i, line) in manifest.split_inclusive('\n').enumerate() {
        if i == idx {
            let eol = if line.ends_with("\r\n") {
                "\r\n"
            } else if line.ends_with('\n') {
                "\n"
            } else {
                ""
            };
            out.push_str(&format!("version = \"{new}\"{eol}"));
        } else {
            out.push_str(line);
        }
    }
    Ok(out)
}

/// Semver without build metadata: `MAJOR.MINOR.PATCH` plus an optional `-pre.release` tag.
pub fn validate(v: &str) -> Result<(), String> {
    let bad = || Err(format!("`{v}` is not a version like 1.2.3 or 1.2.3-rc.1"));
    let (core, pre) = match v.split_once('-') {
        Some((c, p)) => (c, Some(p)),
        None => (v, None),
    };
    let nums: Vec<&str> = core.split('.').collect();
    if nums.len() != 3 || nums.iter().any(|n| n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) || (n.len() > 1 && n.starts_with('0'))) {
        return bad();
    }
    if let Some(p) = pre
        && (p.is_empty() || p.split('.').any(|id| id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')))
    {
        return bad();
    }
    Ok(())
}

/// `CHANGELOG.md` ready for release `version`: the lines under `## Unreleased` move to the top of
/// `## version` (made under Unreleased when it is new), leaving Unreleased empty. The app shows
/// these notes in its What's New window, so a release without them is refused.
pub fn fold_changelog(text: &str, version: &str) -> Result<String, String> {
    const UNRELEASED: &str = "## Unreleased";
    let lines: Vec<&str> = text.lines().collect();
    let heading = |l: &str| l.starts_with("## ");
    let is_version = |l: &str| l.strip_prefix("## ").and_then(|h| h.split_whitespace().next()) == Some(version);
    let Some(u) = lines.iter().position(|l| l.trim_end() == UNRELEASED) else {
        return Err("CHANGELOG.md has no `## Unreleased` section".into());
    };
    let u_end = lines.iter().skip(u + 1).position(|l| heading(l)).map_or(lines.len(), |i| u + 1 + i);
    let notes: Vec<&str> = lines.get(u + 1..u_end).unwrap_or(&[]).iter().copied().skip_while(|l| l.trim().is_empty()).collect();
    let notes: Vec<&str> = notes.iter().rev().copied().skip_while(|l| l.trim().is_empty()).collect::<Vec<_>>().into_iter().rev().collect();
    let existing = lines.iter().position(|l| is_version(l));
    if notes.is_empty() {
        return match existing {
            Some(_) => Ok(text.to_string()),
            None => Err(format!("CHANGELOG.md has no notes for {version}: write the release's changes under `## Unreleased` first")),
        };
    }
    let mut out: Vec<&str> = Vec::with_capacity(lines.len() + 4);
    match existing {
        Some(v) => {
            for (i, l) in lines.iter().enumerate() {
                if i > u && i < u_end {
                    continue;
                }
                out.push(l);
                if i == u {
                    out.push("");
                }
                if i == v {
                    out.push("");
                    out.extend(notes.iter().copied());
                }
            }
        }
        None => {
            for (i, l) in lines.iter().enumerate() {
                out.push(l);
                if i == u {
                    out.push("");
                    out.push("");
                }
            }
            // The heading goes between Unreleased and its notes.
            let at = out.iter().position(|l| l.trim_end() == UNRELEASED).map_or(0, |p| p + 2);
            let heading = format!("## {version}");
            let mut v: Vec<String> = out.iter().map(|s| s.to_string()).collect();
            v.insert(at, heading);
            return Ok(tidy(&v.join("\n")));
        }
    }
    Ok(tidy(&out.join("\n")))
}

/// No runs of blank lines, one newline at the end.
fn tidy(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 1);
    let mut blank = false;
    for l in text.lines() {
        if l.trim().is_empty() {
            if !blank {
                out.push('\n');
            }
            blank = true;
        } else {
            out.push_str(l);
            out.push('\n');
            blank = false;
        }
    }
    out
}

/// Index and text of the `version` line in `[workspace.package]`.
fn find_line(manifest: &str) -> Result<(usize, &str), String> {
    let mut in_section = false;
    for (i, line) in manifest.lines().enumerate() {
        let t = line.trim();
        if t.starts_with('[') {
            in_section = t == "[workspace.package]";
            continue;
        }
        if in_section
            && let Some(rest) = t.strip_prefix("version")
            && rest.trim_start().starts_with('=')
        {
            return Ok((i, line));
        }
    }
    Err("no `version = \"…\"` in [workspace.package] of the root Cargo.toml".into())
}

pub fn run(root: &Path, args: &[&str]) -> Result<(), String> {
    let path = root.join("Cargo.toml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    match args {
        [] => {
            println!("{}", read(&text)?);
            Ok(())
        }
        ["set", new] => {
            let old = read(&text)?;
            let updated = replace(&text, new)?;
            // The release notes first: a version with none is refused before anything is written.
            let log_path = root.join("CHANGELOG.md");
            let log = std::fs::read_to_string(&log_path).map_err(|e| format!("read {}: {e}", log_path.display()))?;
            let folded = fold_changelog(&log, new)?;
            // Write atomically: a half-written root manifest breaks every build in the workspace.
            let tmp = root.join("target").join("Cargo.toml.xtask-version");
            std::fs::create_dir_all(root.join("target")).map_err(|e| format!("create target/: {e}"))?;
            std::fs::write(&tmp, updated).map_err(|e| format!("write {}: {e}", tmp.display()))?;
            std::fs::rename(&tmp, &path).map_err(|e| format!("replace {}: {e}", path.display()))?;
            if folded != log {
                std::fs::write(&log_path, folded).map_err(|e| format!("write {}: {e}", log_path.display()))?;
                println!("CHANGELOG.md: the Unreleased notes are now under {new}");
            }
            // Refresh the workspace members' entries in Cargo.lock (no dependency upgrades).
            let mut c = crate::cargo();
            c.args(["update", "--workspace", "--offline"]);
            if crate::run(c, "cargo update --workspace --offline").is_err() {
                let mut c = crate::cargo();
                c.args(["update", "--workspace"]);
                crate::run(c, "cargo update --workspace")?;
            }
            println!("version: {old} -> {new}");
            Ok(())
        }
        _ => Err("usage: cargo xtask version [set X.Y.Z[-pre]]".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = "[workspace]\nmembers = [\"a\"]\n\n[workspace.package]\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace.dependencies]\nfoo = { version = \"1\" }\n";

    #[test]
    fn the_unreleased_notes_become_the_release() {
        let log = "# Log\n\n## Unreleased\n\n- new thing\n\n## 1.0.0\n\n- old\n";
        assert_eq!(fold_changelog(log, "1.1.0").unwrap(), "# Log\n\n## Unreleased\n\n## 1.1.0\n\n- new thing\n\n## 1.0.0\n\n- old\n");
        // A version already named in the file takes the notes at its top.
        let log = "# Log\n\n## Unreleased\n\n- newer\n\n## 1.1.0\n\n- new thing\n";
        assert_eq!(fold_changelog(log, "1.1.0").unwrap(), "# Log\n\n## Unreleased\n\n## 1.1.0\n\n- newer\n\n- new thing\n");
        // Nothing to move: fine when the version has notes, refused when it has none.
        let log = "## Unreleased\n\n## 1.1.0\n\n- x\n";
        assert_eq!(fold_changelog(log, "1.1.0").unwrap(), log);
        assert!(fold_changelog(log, "1.2.0").is_err());
        assert!(fold_changelog("## 1.0.0\n- x\n", "1.0.0").is_err(), "no Unreleased section");
    }

    #[test]
    fn reads_the_workspace_version() {
        assert_eq!(read(MANIFEST).unwrap(), "0.1.0");
        assert!(read("[package]\nversion = \"1.0.0\"\n").is_err());
    }

    #[test]
    fn replaces_only_the_workspace_version() {
        let out = replace(MANIFEST, "1.2.3-rc.1").unwrap();
        assert_eq!(read(&out).unwrap(), "1.2.3-rc.1");
        assert_eq!(out, MANIFEST.replace("version = \"0.1.0\"", "version = \"1.2.3-rc.1\""));
        assert!(out.contains("foo = { version = \"1\" }"));
    }

    #[test]
    fn keeps_crlf_line_endings() {
        let crlf = MANIFEST.replace('\n', "\r\n");
        let out = replace(&crlf, "2.0.0").unwrap();
        assert_eq!(out, crlf.replace("\"0.1.0\"", "\"2.0.0\""));
    }

    #[test]
    fn validates_versions() {
        for ok in ["0.1.0", "1.20.300", "1.0.0-rc.1", "1.0.0-alpha", "1.0.0-x-y.2"] {
            assert!(validate(ok).is_ok(), "{ok}");
        }
        for bad in ["1.0", "1.0.0.0", "v1.0.0", "01.0.0", "1.0.0-", "1.0.0-a..b", "1.0.0+meta", "1.a.0", ""] {
            assert!(validate(bad).is_err(), "{bad}");
        }
        assert!(replace(MANIFEST, "nope").is_err());
    }

    #[test]
    fn the_real_manifest_has_a_valid_version() {
        let text = std::fs::read_to_string(crate::root().join("Cargo.toml")).unwrap();
        validate(&read(&text).unwrap()).unwrap();
    }
}
