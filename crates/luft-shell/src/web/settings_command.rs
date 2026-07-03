pub fn settings_command(command: &str, page: &str) -> String {
    let command = command.trim();
    if command.is_empty() {
        return String::new();
    }
    let page = page.trim();
    if page.is_empty() {
        command.to_string()
    } else {
        format!("{command} {page}")
    }
}
