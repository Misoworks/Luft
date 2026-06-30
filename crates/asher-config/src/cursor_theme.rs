pub const DEFAULT_CURSOR_THEME_NAME: &str = "macOS";
pub const DEFAULT_CURSOR_THEME_PARENT: &str = "/home/kristof/Downloads/macOS";
pub const DEFAULT_CURSOR_THEME_DIR: &str = "/home/kristof/Downloads/macOS/macOS";
pub const DEFAULT_CURSOR_SIZE: &str = "24";

pub fn cursor_environment_entries() -> [(&'static str, &'static str); 4] {
    [
        ("XCURSOR_THEME", DEFAULT_CURSOR_THEME_NAME),
        ("XCURSOR_SIZE", DEFAULT_CURSOR_SIZE),
        ("XCURSOR_PATH", DEFAULT_CURSOR_THEME_PARENT),
        ("ASHER_CURSOR_THEME_DIR", DEFAULT_CURSOR_THEME_DIR),
    ]
}
