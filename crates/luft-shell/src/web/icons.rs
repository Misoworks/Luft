use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
use luft_ipc::WindowSummary;

static ICON_URI_CACHE: OnceLock<Mutex<HashMap<PathBuf, Option<String>>>> = OnceLock::new();
const BASE64_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn window_icon_uri(window: &WindowSummary) -> Option<String> {
    window
        .icon_uri
        .clone()
        .or_else(|| {
            window
                .icon_name
                .as_deref()
                .and_then(|icon| crate::apps::resolve_icon_path(Some(icon)))
                .as_deref()
                .and_then(icon_data_uri)
        })
        .or_else(|| {
            window
                .app_id
                .as_deref()
                .and_then(|app_id| crate::apps::resolve_icon_path(Some(app_id)))
                .as_deref()
                .and_then(icon_data_uri)
        })
}

pub fn icon_data_uri(path: &Path) -> Option<String> {
    let cache = ICON_URI_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(cache) = cache.lock()
        && let Some(uri) = cache.get(path)
    {
        return uri.clone();
    }

    let uri = icon_data_uri_uncached(path);
    if let Ok(mut cache) = cache.lock() {
        cache.insert(path.to_path_buf(), uri.clone());
    }
    uri
}

fn icon_data_uri_uncached(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let mime = icon_mime(path, &bytes)?;
    Some(bytes_data_uri(mime, &bytes))
}

pub(crate) fn bytes_data_uri(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", base64_encode(bytes))
}

fn icon_mime(path: &Path, bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return Some("image/webp");
    }
    if bytes
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
        == Some(b'<')
    {
        return Some("image/svg+xml");
    }

    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("svg") => Some("image/svg+xml"),
        Some("png") => Some("image/png"),
        Some("jpg" | "jpeg") => Some("image/jpeg"),
        Some("webp") => Some("image/webp"),
        _ => None,
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        let value = ((first as u32) << 16) | ((second as u32) << 8) | third as u32;

        encoded.push(BASE64_TABLE[((value >> 18) & 0x3f) as usize] as char);
        encoded.push(BASE64_TABLE[((value >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            encoded.push(BASE64_TABLE[((value >> 6) & 0x3f) as usize] as char);
        } else {
            encoded.push('=');
        }
        if chunk.len() > 2 {
            encoded.push(BASE64_TABLE[(value & 0x3f) as usize] as char);
        } else {
            encoded.push('=');
        }
    }
    encoded
}
