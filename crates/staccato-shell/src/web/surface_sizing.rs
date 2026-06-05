use super::model::WebShellSnapshot;

pub(crate) const QUICK_SETTINGS_WIDTH: i32 = 420;
pub(crate) const NOTIFICATION_TOAST_WIDTH: i32 = 380;
pub(crate) const NOTIFICATION_TOAST_HEIGHT: i32 = 136;

pub(crate) fn quick_settings_size(snapshot: &WebShellSnapshot) -> (i32, i32) {
    let status_tiles = 1
        + i32::from(snapshot.status.network.is_some())
        + i32::from(snapshot.status.battery.is_some());
    let status_rows = (status_tiles + 1) / 2;
    let sliders = i32::from(snapshot.status.audio.is_some())
        + i32::from(snapshot.status.brightness.is_some());
    let mut height = 32 + 38 + 14 + status_rows * 58 + (status_rows - 1) * 10;
    if sliders > 0 {
        height += 14 + sliders * 58 + (sliders - 1) * 10;
    }
    (QUICK_SETTINGS_WIDTH, height.max(230))
}

pub(crate) fn notification_toast_size() -> (i32, i32) {
    (NOTIFICATION_TOAST_WIDTH, NOTIFICATION_TOAST_HEIGHT)
}
