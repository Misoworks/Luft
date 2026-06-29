use std::{
    env, fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let Some(workspace_root) = manifest_dir.parent().and_then(|path| path.parent()) else {
        return;
    };

    println!("cargo:rerun-if-changed=../kestrel/src");
    println!("cargo:rerun-if-changed=../kestrel/Cargo.toml");
    println!("cargo:rerun-if-changed=../asher-shell/src");
    println!("cargo:rerun-if-changed=../asher-shell/Cargo.toml");
    println!("cargo:rerun-if-changed=../asher-ipc/src");

    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let target_dir = target_dir(&profile);
    let kestrel = target_dir.join("kestrel");
    let shell = target_dir.join("asher-shell");

    let watch_paths = [
        workspace_root.join("crates/kestrel/src"),
        workspace_root.join("crates/kestrel/Cargo.toml"),
        workspace_root.join("crates/asher-shell/src"),
        workspace_root.join("crates/asher-shell/Cargo.toml"),
        workspace_root.join("crates/asher-ipc/src"),
    ];

    if needs_rebuild(&kestrel, &watch_paths) || needs_rebuild(&shell, &watch_paths) {
        println!(
            "cargo:warning=kestrel/asher-shell may be stale; build them with `cargo build -p kestrel -p asher-shell` before running asher-session"
        );
    }
}

fn target_dir(profile: &str) -> PathBuf {
    if let Ok(dir) = env::var("CARGO_TARGET_DIR") {
        return PathBuf::from(dir).join(profile);
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must be set"));
    let mut path = out_dir;
    for _ in 0..3 {
        path.pop();
    }
    path
}

fn needs_rebuild(binary: &Path, watch_paths: &[PathBuf]) -> bool {
    let Ok(binary_mtime) = binary_metadata(binary) else {
        return true;
    };

    watch_paths
        .iter()
        .any(|path| path_modified_after(path, binary_mtime))
}

fn binary_metadata(path: &Path) -> Result<SystemTime, ()> {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map_err(|_| ())
}

fn path_modified_after(path: &Path, cutoff: SystemTime) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };

    if metadata
        .modified()
        .ok()
        .is_some_and(|modified| modified > cutoff)
    {
        return true;
    }

    if !metadata.is_dir() {
        return false;
    }

    let Ok(read_dir) = fs::read_dir(path) else {
        return false;
    };

    read_dir.filter_map(Result::ok).any(|entry| {
        let entry_path = entry.path();
        if entry_path.is_dir() {
            path_modified_after(&entry_path, cutoff)
        } else {
            entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .ok()
                .is_some_and(|modified| modified > cutoff)
        }
    })
}
