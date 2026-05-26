use staccato_layout::ModeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellChrome {
    pub panel: bool,
    pub dock: bool,
    pub sidebar: bool,
}

impl ShellChrome {
    pub fn for_mode(mode: ModeId) -> Self {
        match mode {
            ModeId::Browser => Self {
                panel: true,
                dock: false,
                sidebar: true,
            },
            ModeId::Dock => Self {
                panel: true,
                dock: true,
                sidebar: false,
            },
            ModeId::Focus => Self {
                panel: true,
                dock: false,
                sidebar: false,
            },
            ModeId::Panel => Self {
                panel: true,
                dock: false,
                sidebar: false,
            },
            ModeId::Classic | ModeId::Tiling | ModeId::Tablet => Self {
                panel: true,
                dock: false,
                sidebar: false,
            },
        }
    }
}
