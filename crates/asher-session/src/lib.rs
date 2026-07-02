use serde::{Deserialize, Serialize};
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionDescriptor {
    pub name: String,
    pub comment: String,
    pub exec: String,
    #[serde(default = "default_try_exec")]
    pub try_exec: String,
    pub desktop_names: String,
    #[serde(default = "default_keywords")]
    pub keywords: Vec<String>,
}

impl Default for SessionDescriptor {
    fn default() -> Self {
        Self {
            name: "Asher".to_string(),
            comment: "Asher Desktop Environment".to_string(),
            exec: "asher-session --session".to_string(),
            try_exec: default_try_exec(),
            desktop_names: "Asher".to_string(),
            keywords: default_keywords(),
        }
    }
}

impl SessionDescriptor {
    pub fn desktop_entry(&self) -> String {
        let mut entry = format!(
            "[Desktop Entry]\nName={}\nComment={}\nExec={}\nTryExec={}\nType=Application\nDesktopNames={}\n",
            self.name, self.comment, self.exec, self.try_exec, self.desktop_names
        );
        let keywords = desktop_entry_list(&self.keywords);
        if !keywords.is_empty() {
            entry.push_str("Keywords=");
            entry.push_str(&keywords);
            entry.push('\n');
        }
        entry
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionEnvironment {
    pub xdg_current_desktop: String,
    pub xdg_session_desktop: String,
    pub desktop_session: String,
    pub xdg_session_type: String,
}

impl Default for SessionEnvironment {
    fn default() -> Self {
        Self {
            xdg_current_desktop: "Asher".to_string(),
            xdg_session_desktop: "asher".to_string(),
            desktop_session: "asher".to_string(),
            xdg_session_type: "wayland".to_string(),
        }
    }
}

impl SessionEnvironment {
    pub fn entries(&self) -> Vec<(&'static str, &str)> {
        let mut entries = vec![
            ("XDG_CURRENT_DESKTOP", self.xdg_current_desktop.as_str()),
            ("XDG_SESSION_DESKTOP", self.xdg_session_desktop.as_str()),
            ("DESKTOP_SESSION", self.desktop_session.as_str()),
            ("XDG_SESSION_TYPE", self.xdg_session_type.as_str()),
            ("GDK_BACKEND", "wayland,x11"),
            ("QT_QPA_PLATFORM", "wayland;xcb"),
            ("SDL_VIDEODRIVER", "wayland"),
            ("CLUTTER_BACKEND", "wayland"),
            ("MOZ_ENABLE_WAYLAND", "1"),
            ("ELECTRON_OZONE_PLATFORM_HINT", "auto"),
            ("NO_AT_BRIDGE", "1"),
        ];
        entries.extend(asher_config::cursor_environment_entries());
        entries
    }

    pub fn apply_to_command(&self, command: &mut Command) {
        for (name, value) in self.entries() {
            command.env(name, value);
        }
    }
}

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("session descriptor is missing {field}")]
    MissingField { field: &'static str },
    #[error("session descriptor field {field} must not contain newlines")]
    InvalidField { field: &'static str },
}

pub fn validate_descriptor(descriptor: &SessionDescriptor) -> Result<(), SessionError> {
    validate_required("name", &descriptor.name)?;
    validate_required("comment", &descriptor.comment)?;
    validate_required("exec", &descriptor.exec)?;
    validate_required("try_exec", &descriptor.try_exec)?;
    validate_required("desktop_names", &descriptor.desktop_names)?;
    for keyword in &descriptor.keywords {
        validate_desktop_entry_value("keywords", keyword)?;
    }
    Ok(())
}

fn validate_required(field: &'static str, value: &str) -> Result<(), SessionError> {
    if value.trim().is_empty() {
        return Err(SessionError::MissingField { field });
    }
    validate_desktop_entry_value(field, value)
}

fn validate_desktop_entry_value(field: &'static str, value: &str) -> Result<(), SessionError> {
    if value.contains('\n') || value.contains('\r') {
        return Err(SessionError::InvalidField { field });
    }
    Ok(())
}

fn desktop_entry_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| format!("{value};"))
        .collect()
}

fn default_try_exec() -> String {
    "asher-session".to_string()
}

fn default_keywords() -> Vec<String> {
    ["wayland", "desktop", "session"]
        .into_iter()
        .map(ToString::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const INSTALLED_DESKTOP_ENTRY: &str = include_str!("../../../data/sessions/asher.desktop");

    #[test]
    fn default_descriptor_matches_installed_desktop_entry() {
        assert_eq!(
            SessionDescriptor::default().desktop_entry(),
            INSTALLED_DESKTOP_ENTRY
        );
    }

    #[test]
    fn default_descriptor_is_valid() {
        validate_descriptor(&SessionDescriptor::default()).unwrap();
    }

    #[test]
    fn desktop_entry_formats_keywords_as_desktop_entry_list() {
        let descriptor = SessionDescriptor {
            keywords: vec!["wayland".to_string(), "session".to_string()],
            ..SessionDescriptor::default()
        };

        assert!(
            descriptor
                .desktop_entry()
                .contains("Keywords=wayland;session;\n")
        );
    }

    #[test]
    fn validation_rejects_missing_desktop_names() {
        let descriptor = SessionDescriptor {
            desktop_names: String::new(),
            ..SessionDescriptor::default()
        };

        assert!(matches!(
            validate_descriptor(&descriptor),
            Err(SessionError::MissingField {
                field: "desktop_names"
            })
        ));
    }

    #[test]
    fn validation_rejects_injected_newline() {
        let descriptor = SessionDescriptor {
            name: "Asher\nExec=bad".to_string(),
            ..SessionDescriptor::default()
        };

        assert!(matches!(
            validate_descriptor(&descriptor),
            Err(SessionError::InvalidField { field: "name" })
        ));
    }
}
