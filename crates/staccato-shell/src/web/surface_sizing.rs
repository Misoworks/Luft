use super::model::WebShellSnapshot;

pub(crate) const QUICK_SETTINGS_WIDTH: i32 = 420;
pub(crate) const NOTIFICATION_TOAST_WIDTH: i32 = 380;
pub(crate) const NOTIFICATION_TOAST_HEIGHT: i32 = 136;

pub(crate) fn quick_settings_size(snapshot: &WebShellSnapshot) -> (i32, i32) {
    let status_tiles =
        i32::from(snapshot.status.network.is_some()) + i32::from(snapshot.status.battery.is_some());
    let sliders = i32::from(snapshot.status.audio.is_some())
        + i32::from(snapshot.status.brightness.is_some());
    let mut height = 32 + 36 + 14 + 44;
    if status_tiles > 0 {
        height += 14 + 56;
    }
    if sliders > 0 {
        height += 14 + sliders * 58 + (sliders - 1) * 10;
    }
    (QUICK_SETTINGS_WIDTH, height.max(176))
}

pub(crate) fn notification_toast_size() -> (i32, i32) {
    (NOTIFICATION_TOAST_WIDTH, NOTIFICATION_TOAST_HEIGHT)
}
