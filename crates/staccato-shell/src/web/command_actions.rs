use super::WebShell;
use staccato_config::ConfigPaths;
use tracing::warn;

impl WebShell {
    pub(super) fn reload_config_from_command(&mut self) {
        self.hide_chrome();
        self.reload_shell_config();
    }

    pub(super) fn open_logs_folder(&mut self) {
        self.hide_chrome();
        let file_manager = self.config.default_apps.file_manager.trim();
        if file_manager.is_empty() {
            warn!("cannot open logs folder without a configured file manager");
            return;
        }
        match ConfigPaths::discover() {
            Ok(paths) => {
                let logs_dir = paths.logs_dir();
                if let Err(error) = std::fs::create_dir_all(&logs_dir) {
                    warn!(%error, path = %logs_dir.display(), "failed to create Staccato logs directory");
                    return;
                }
                self.launch(format!(
                    "{file_manager} {}",
                    shell_quote(&logs_dir.display().to_string())
                ));
            }
            Err(error) => warn!(%error, "failed to locate Staccato logs directory"),
        }
    }

    pub(super) fn toggle_safe_mode(&mut self) {
        self.hide_chrome();
        let mut config = self.config.clone();
        config.general.safe_mode = !config.general.safe_mode;
        self.save_shell_config(config);
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
