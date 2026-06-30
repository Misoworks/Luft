use asher_ipc::ModeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellChrome {
    pub panel: bool,
}

impl ShellChrome {
    pub fn for_mode(_mode: ModeId) -> Self {
        Self { panel: true }
    }
}
