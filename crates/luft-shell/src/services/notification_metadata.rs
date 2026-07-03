use super::notifications::{NotificationAction, NotificationUrgency};
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};
use zbus::zvariant::OwnedValue;

pub(super) fn current_unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

pub(super) fn urgency_from_hints(hints: &HashMap<String, OwnedValue>) -> NotificationUrgency {
    match hints
        .get("urgency")
        .and_then(|value| u8::try_from(value.clone()).ok())
    {
        Some(0) => NotificationUrgency::Low,
        Some(2) => NotificationUrgency::Critical,
        _ => NotificationUrgency::Normal,
    }
}

pub(super) fn action_pairs(actions: Vec<String>) -> Vec<NotificationAction> {
    actions
        .chunks(2)
        .filter_map(|pair| {
            let key = pair.first()?.trim();
            let label = pair.get(1)?.trim();
            (!key.is_empty() && !label.is_empty()).then(|| NotificationAction {
                key: key.to_string(),
                label: strip_markup(label),
            })
        })
        .collect()
}

pub(super) fn clean_icon_name(icon: &str) -> Option<String> {
    let icon = icon.trim();
    (!icon.is_empty()).then(|| icon.to_string())
}

pub(super) fn clean_app_name(app_name: &str) -> String {
    let app_name = app_name.trim();
    if app_name.is_empty() {
        "Application".to_string()
    } else {
        app_name.to_string()
    }
}

pub(super) fn strip_markup(text: &str) -> String {
    let mut output = String::new();
    let mut inside_tag = false;
    for character in text.chars() {
        match character {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => output.push(character),
            _ => {}
        }
    }
    output
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}
