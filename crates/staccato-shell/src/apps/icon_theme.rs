use super::xdg;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

static ICON_CACHE: OnceLock<Mutex<HashMap<String, Option<PathBuf>>>> = OnceLock::new();

pub(crate) fn resolve_icon_path(icon: Option<&str>) -> Option<PathBuf> {
    let icon = icon?.trim();
    if icon.is_empty() {
        return None;
    }

    let cache = ICON_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(cache) = cache.lock()
        && let Some(cached) = cache.get(icon)
    {
        return cached.clone();
    }

    let resolved = resolve_icon_path_uncached(icon);
    if let Ok(mut cache) = cache.lock() {
        cache.insert(icon.to_string(), resolved.clone());
    }

    resolved
}

fn resolve_icon_path_uncached(icon: &str) -> Option<PathBuf> {
    let path = PathBuf::from(icon);
    if path.is_absolute() && path.exists() {
        return Some(path);
    }

    let icons = icon_names(icon)
        .into_iter()
        .filter(|name| !name.ends_with("-symbolic"))
        .collect::<Vec<_>>();
    for dir in xdg::data_dirs() {
        for icon in &icons {
            for candidate in icon_candidates(&dir, icon) {
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }

    None
}

fn icon_names(icon: &str) -> Vec<String> {
    let path = Path::new(icon);
    let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
        return vec![icon.to_string()];
    };

    let base = if path.extension().is_some() {
        vec![stem.to_string(), icon.to_string()]
    } else {
        vec![icon.to_string()]
    };
    let mut names = Vec::new();
    for name in base {
        push_icon_name(&mut names, name.clone());
        if let Some(first) = name.chars().next() {
            let title = first.to_uppercase().chain(name.chars().skip(1)).collect();
            push_icon_name(&mut names, title);
        }
    }
    names
}

fn push_icon_name(names: &mut Vec<String>, name: String) {
    if !names.contains(&name) {
        names.push(name);
    }
}

fn icon_candidates(dir: &Path, icon: &str) -> Vec<PathBuf> {
    let sizes = [
        "512x512", "256x256", "128x128", "96x96", "64x64", "48x48", "32x32", "24x24", "22x22",
        "16x16",
    ];
    let sections = [
        "apps",
        "legacy",
        "actions",
        "devices",
        "places",
        "status",
        "mimetypes",
    ];
    let themes = icon_theme_dirs(dir);
    let mut candidates = Vec::new();
    for theme in &themes {
        for extension in ["svg", "png"] {
            for section in sections {
                candidates.push(
                    theme
                        .join("scalable")
                        .join(section)
                        .join(format!("{icon}.{extension}")),
                );
            }
        }
    }
    for theme in &themes {
        for size in sizes {
            for section in sections {
                for extension in ["png", "svg"] {
                    candidates.push(
                        theme
                            .join(size)
                            .join(section)
                            .join(format!("{icon}.{extension}")),
                    );
                }
            }
        }
    }
    candidates.push(dir.join("pixmaps").join(format!("{icon}.png")));
    candidates.push(dir.join("pixmaps").join(format!("{icon}.svg")));
    candidates
}

fn icon_theme_dirs(dir: &Path) -> Vec<PathBuf> {
    let icons_dir = dir.join("icons");
    let preferred = [
        "Papirus",
        "Papirus-Dark",
        "Papirus-Light",
        "breeze",
        "breeze-dark",
        "hicolor",
        "Adwaita",
        "AdwaitaLegacy",
        "Yaru",
    ];
    let mut themes = preferred
        .iter()
        .map(|theme| icons_dir.join(theme))
        .filter(|path| path.exists())
        .collect::<Vec<_>>();

    let Ok(entries) = fs::read_dir(&icons_dir) else {
        return themes;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && !themes.contains(&path) {
            themes.push(path);
        }
    }
    themes
}
