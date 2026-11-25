//! `cargo xtask`: PingCraft's gates and chores (ADR 0009).
//!
//! - `ci`: everything CI runs, in order, with an ok/FAIL summary.
//! - `layers`: checks the crate layering (nothing below the UI depends on egui; the UI never sees Matrix SDK types).
//! - `bump-images [--check]`: moves the digests pinned in `deploy/compose.yaml` to what each pinned tag points at now.
//! - `version set <version>`: sets the workspace version (Cargo.toml and the workspace's own entries in Cargo.lock),
//!   as craft-common-actions' release workflow asks every Craft app to.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

type Result<T> = std::result::Result<T, String>;
/// One gate of `cargo xtask ci`.
type Step = Box<dyn Fn() -> Result<()>>;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("ci") => ci(),
        Some("layers") => layers(),
        Some("bump-images") => bump_images(args.iter().any(|a| a == "--check")),
        Some("version") if args.get(1).map(String::as_str) == Some("set") && args.len() == 3 => set_version(&args[2]),
        _ => {
            eprintln!("usage: cargo xtask <ci | layers | bump-images [--check] | version set <version>>");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask: {e}");
            ExitCode::FAILURE
        }
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
}

fn run(program: &str, args: &[&str]) -> Result<()> {
    println!("$ {program} {}", args.join(" "));
    let status = Command::new(program).args(args).current_dir(root()).status().map_err(|e| format!("could not run {program}: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{program} {} failed ({status})", args.join(" "))) }
}

// ---------------------------------------------------------------------------------------------------------------
// ci

/// The crates checked for the web. The desktop app and the keychain are native only.
const WASM_CRATES: &[&str] = &["pingcraft-core", "pingcraft-matrix", "pingcraft-ui-egui", "pingcraft-web"];

fn ci() -> Result<()> {
    let mut wasm_args = vec!["clippy", "--locked", "--target", "wasm32-unknown-unknown"];
    for c in WASM_CRATES {
        wasm_args.extend(["-p", c]);
    }
    wasm_args.extend(["--", "-D", "warnings"]);

    let steps: Vec<(&str, Step)> = vec![
        ("fmt", Box::new(|| run("cargo", &["fmt", "--all", "--check"]))),
        ("clippy", Box::new(|| run("cargo", &["clippy", "--locked", "--workspace", "--all-targets", "--", "-D", "warnings"]))),
        ("test", Box::new(|| run("cargo", &["test", "--locked", "--workspace"]))),
        ("layers", Box::new(layers)),
        ("wasm", Box::new(move || run("cargo", &wasm_args))),
        // Licences, advisories and sources (deny.toml). CI may also run it as its own job.
        ("deny", Box::new(|| run("cargo", &["deny", "--locked", "check"]))),
    ];
    let mut failed = Vec::new();
    let mut summary = Vec::new();
    for (name, step) in &steps {
        println!("\n== {name}");
        match step() {
            Ok(()) => summary.push(format!("ok    {name}")),
            Err(e) => {
                summary.push(format!("FAIL  {name}: {e}"));
                failed.push(*name);
            }
        }
    }
    println!("\n{}", summary.join("\n"));
    if failed.is_empty() { Ok(()) } else { Err(format!("failed: {}", failed.join(", "))) }
}

// ---------------------------------------------------------------------------------------------------------------
// layers

/// Every workspace crate and its layer. A crate may depend only on crates in lower layers. A crate missing here
/// fails the check, so a new crate has to be placed deliberately.
const LAYERS: &[(&str, u8)] = &[
    ("pingcraft-core", 0),
    ("pingcraft-keychain", 0),
    ("pingcraft-matrix", 1),
    ("pingcraft-notify", 1),
    ("pingcraft-ui-egui", 2),
    ("pingcraft", 3),
    ("pingcraft-web", 3),
    ("xtask", 9),
];

/// UI and windowing crates: only the UI layer and the apps may use them.
const UI_ONLY: &[&str] = &["egui", "eframe", "egui_kittest", "egui-wgpu", "egui-winit", "winit", "craft-ui"];
/// The UI layer is below the Matrix adapter in what it may see: it works on `pingcraft-core` types only.
const NOT_IN_UI: &[&str] = &["pingcraft-matrix", "matrix-sdk", "matrix-sdk-ui", "matrix-sdk-base", "matrix-sdk-crypto", "ruma"];
const UI_LAYER: u8 = 2;

fn layers() -> Result<()> {
    let out = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
        .current_dir(root())
        .output()
        .map_err(|e| format!("could not run cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(format!("cargo metadata failed: {}", String::from_utf8_lossy(&out.stderr)));
    }
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| format!("cargo metadata output: {e}"))?;
    let deps = workspace_deps(&meta)?;
    let problems = layer_problems(&deps);
    if problems.is_empty() {
        println!("layers: {} crates, all edges point down", deps.len());
        Ok(())
    } else {
        Err(format!("layering broken:\n  {}", problems.join("\n  ")))
    }
}

/// Workspace crate name -> the names of everything it depends on (normal, build and target-specific).
fn workspace_deps(meta: &serde_json::Value) -> Result<BTreeMap<String, Vec<String>>> {
    let packages = meta.get("packages").and_then(|p| p.as_array()).ok_or("cargo metadata: no packages")?;
    let mut map = BTreeMap::new();
    for p in packages {
        let name = p.get("name").and_then(|n| n.as_str()).ok_or("cargo metadata: package without a name")?;
        let deps = p
            .get("dependencies")
            .and_then(|d| d.as_array())
            .map(|ds| {
                ds.iter()
                    .filter(|d| d.get("kind").and_then(|k| k.as_str()) != Some("dev"))
                    .filter_map(|d| d.get("name").and_then(|n| n.as_str()).map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        map.insert(name.to_owned(), deps);
    }
    Ok(map)
}

fn layer_problems(deps: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let layer_of = |name: &str| LAYERS.iter().find(|(n, _)| *n == name).map(|(_, l)| *l);
    let mut problems = Vec::new();
    for (krate, krate_deps) in deps {
        let Some(layer) = layer_of(krate) else {
            problems.push(format!("{krate} has no layer: add it to LAYERS in xtask/src/main.rs"));
            continue;
        };
        for dep in krate_deps {
            if let Some(dep_layer) = layer_of(dep)
                && dep_layer >= layer
            {
                problems.push(format!("{krate} (layer {layer}) depends on {dep} (layer {dep_layer}): edges must point down"));
            }
            if layer < UI_LAYER && UI_ONLY.contains(&dep.as_str()) {
                problems.push(format!("{krate} (layer {layer}) uses {dep}: UI crates are for the UI layer and the apps only"));
            }
            if layer == UI_LAYER && NOT_IN_UI.contains(&dep.as_str()) {
                problems.push(format!("{krate} uses {dep}: the UI works on pingcraft-core types only (ADR 0009)"));
            }
        }
    }
    problems
}

// ---------------------------------------------------------------------------------------------------------------
// bump-images

const COMPOSE: &str = "deploy/compose.yaml";

/// One `image: name:tag@sha256:digest` line.
#[derive(Debug, PartialEq, Eq)]
struct PinnedImage {
    reference: String,
    digest: String,
}

fn parse_pinned(line: &str) -> Option<PinnedImage> {
    let rest = line.trim_start().strip_prefix("image:")?.trim();
    let (reference, digest) = rest.split_once('@')?;
    let digest = digest.strip_prefix("sha256:")?;
    if reference.is_empty() || digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(PinnedImage { reference: reference.to_owned(), digest: digest.to_owned() })
}

fn current_digest(reference: &str) -> Result<String> {
    let out = Command::new("docker")
        .args(["buildx", "imagetools", "inspect", reference, "--format", "{{json .Manifest}}"])
        .output()
        .map_err(|e| format!("could not run docker: {e}"))?;
    if !out.status.success() {
        return Err(format!("docker could not inspect {reference}: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let manifest: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| format!("{reference}: {e}"))?;
    manifest
        .get("digest")
        .and_then(|d| d.as_str())
        .and_then(|d| d.strip_prefix("sha256:"))
        .map(str::to_owned)
        .ok_or_else(|| format!("{reference}: no digest in the registry's answer"))
}

/// Updates each pinned digest to what its tag points at now. New versions (tags) are chosen by hand in a pull
/// request; this only catches a tag that was rebuilt, for example with security fixes. `--check` changes nothing
/// and fails when a digest is stale.
fn bump_images(check: bool) -> Result<()> {
    let path = root().join(COMPOSE);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{COMPOSE}: {e}"))?;
    let mut stale = 0;
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let Some(pinned) = parse_pinned(line) else {
            out.push_str(line);
            continue;
        };
        let now = current_digest(&pinned.reference)?;
        if now == pinned.digest {
            println!("up to date  {}", pinned.reference);
            out.push_str(line);
        } else {
            stale += 1;
            println!("stale       {}  {} -> {}", pinned.reference, &pinned.digest[..12], &now[..12.min(now.len())]);
            out.push_str(&line.replace(&pinned.digest, &now));
        }
    }
    if check {
        return if stale == 0 { Ok(()) } else { Err(format!("{stale} image digest(s) stale; run cargo xtask bump-images")) };
    }
    if stale > 0 {
        std::fs::write(&path, out).map_err(|e| format!("{COMPOSE}: {e}"))?;
        println!("updated {stale} digest(s) in {COMPOSE}");
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// version set

/// A version as Cargo takes it: X.Y.Z, optionally with a pre-release such as `-rc.1`.
fn valid_version(v: &str) -> bool {
    let (core, pre) = v.split_once('-').unwrap_or((v, ""));
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        && pre.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        && !(v.contains('-') && pre.is_empty())
}

/// Cargo.toml with `[workspace.package] version` set.
fn with_workspace_version(toml: &str, version: &str) -> Result<String> {
    let mut out = String::with_capacity(toml.len());
    let (mut in_package, mut done) = (false, false);
    for line in toml.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[workspace.package]";
        } else if in_package && !done && trimmed.starts_with("version") && trimmed.contains('=') {
            out.push_str(&format!("version = \"{version}\"\n"));
            done = true;
            continue;
        }
        out.push_str(line);
    }
    if done { Ok(out) } else { Err("Cargo.toml has no [workspace.package] version".into()) }
}

/// Cargo.lock with the version of every workspace package (an entry without a `source`) set.
fn with_lock_versions(lock: &str, members: &[String], version: &str) -> String {
    lock.split("\n[[package]]")
        .map(|block| {
            let name = block.lines().find_map(|l| l.strip_prefix("name = \"")).and_then(|n| n.strip_suffix('"'));
            let ours = name.is_some_and(|n| members.iter().any(|m| m == n)) && !block.contains("\nsource = ");
            if !ours {
                return block.to_owned();
            }
            // Line by line, keeping every line ending (blocks end with the blank line before the next one).
            block
                .split_inclusive('\n')
                .map(|l| {
                    if l.starts_with("version = \"") {
                        format!("version = \"{version}\"{}", if l.ends_with('\n') { "\n" } else { "" })
                    } else {
                        l.to_owned()
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n[[package]]")
}

fn set_version(version: &str) -> Result<()> {
    if !valid_version(version) {
        return Err(format!("not a version: {version} (expected X.Y.Z or X.Y.Z-pre)"));
    }
    let toml_path = root().join("Cargo.toml");
    let toml = std::fs::read_to_string(&toml_path).map_err(|e| format!("Cargo.toml: {e}"))?;
    std::fs::write(&toml_path, with_workspace_version(&toml, version)?).map_err(|e| format!("Cargo.toml: {e}"))?;
    let lock_path = root().join("Cargo.lock");
    let lock = std::fs::read_to_string(&lock_path).map_err(|e| format!("Cargo.lock: {e}"))?;
    let members: Vec<String> = LAYERS.iter().map(|(name, _)| (*name).to_owned()).collect();
    std::fs::write(&lock_path, with_lock_versions(&lock, &members, version)).map_err(|e| format!("Cargo.lock: {e}"))?;
    println!("version set to {version}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{PinnedImage, layer_problems, parse_pinned, valid_version, with_lock_versions, with_workspace_version};
    use std::collections::BTreeMap;

    fn deps(pairs: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), v.iter().map(|s| (*s).to_owned()).collect())).collect()
    }

    #[test]
    fn the_intended_layering_passes() {
        let d = deps(&[
            ("pingcraft-core", &[]),
            ("pingcraft-matrix", &["pingcraft-core", "matrix-sdk"]),
            ("pingcraft-ui-egui", &["pingcraft-core", "egui"]),
            ("pingcraft", &["pingcraft-ui-egui", "pingcraft-matrix", "eframe"]),
        ]);
        assert!(layer_problems(&d).is_empty(), "{:?}", layer_problems(&d));
    }

    #[test]
    fn the_ui_may_not_see_the_matrix_sdk() {
        let d = deps(&[("pingcraft-ui-egui", &["pingcraft-matrix"]), ("pingcraft-matrix", &[])]);
        let p = layer_problems(&d);
        // The edge points down (2 -> 1), so only the UI rule catches it.
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(p[0].contains("pingcraft-ui-egui uses pingcraft-matrix"), "{p:?}");
    }

    #[test]
    fn egui_below_the_ui_and_unplaced_crates_fail() {
        let d = deps(&[("pingcraft-core", &["egui"]), ("pingcraft-new", &[])]);
        let p = layer_problems(&d);
        assert!(p.iter().any(|m| m.contains("pingcraft-core (layer 0) uses egui")), "{p:?}");
        assert!(p.iter().any(|m| m.contains("pingcraft-new has no layer")), "{p:?}");
    }

    #[test]
    fn pinned_image_lines_parse() {
        let d = "035b9ab53cfa147d7202b61f5f7782b939ae815b7d6bc81c96b7b42ff1fca950";
        assert_eq!(
            parse_pinned(&format!("    image: postgres:18.2-alpine3.23@sha256:{d}\n")),
            Some(PinnedImage { reference: "postgres:18.2-alpine3.23".into(), digest: d.into() })
        );
        assert_eq!(parse_pinned("    image: postgres:18"), None);
        assert_eq!(parse_pinned("    image: x@sha256:abc"), None);
        assert_eq!(parse_pinned("    # image: x"), None);
    }

    #[test]
    fn version_set_touches_only_the_workspace() {
        assert!(valid_version("0.2.0") && valid_version("1.2.3-rc.1"));
        assert!(!valid_version("1.2") && !valid_version("v1.2.3") && !valid_version("1.2.3-") && !valid_version("1.2.3-a b"));
        let toml = "[workspace]\nmembers = []\n\n[workspace.package]\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace.dependencies]\nx = { version = \"1\" }\n";
        let out = with_workspace_version(toml, "0.2.0").unwrap();
        assert!(out.contains("[workspace.package]\nversion = \"0.2.0\"\n"), "{out}");
        assert!(out.contains("x = { version = \"1\" }"), "dependencies keep theirs");
        assert!(with_workspace_version("[package]\nversion = \"1\"\n", "0.2.0").is_err());

        let lock = "version = 4\n\n[[package]]\nname = \"pingcraft-core\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"serde\"\nversion = \"0.1.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n";
        let out = with_lock_versions(lock, &["pingcraft-core".into()], "0.2.0");
        assert!(out.contains("name = \"pingcraft-core\"\nversion = \"0.2.0\""), "{out}");
        assert!(out.contains("name = \"serde\"\nversion = \"0.1.0\""), "a registry crate keeps its version: {out}");
        assert!(out.starts_with("version = 4\n"), "the lock format version is not touched");
        assert_eq!(with_lock_versions(&out, &["pingcraft-core".into()], "0.1.0"), lock, "setting it back gives the same file");
    }
}
