use serde::Serialize;
use staccato_layout::{ModeId, WindowState, state_for_mode};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModeSummary {
    id: &'static str,
    name: &'static str,
    mode: ModeId,
    default_window_state: WindowState,
}

pub fn list_modes(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let modes = mode_summaries();
    if json {
        println!("{}", serde_json::to_string_pretty(&modes)?);
    } else {
        for mode in modes {
            println!(
                "{}\t{}\t{:?}",
                mode.id, mode.name, mode.default_window_state
            );
        }
    }
    Ok(())
}

fn mode_summaries() -> Vec<ModeSummary> {
    [
        ("classic", "Classic", ModeId::Classic),
        ("dock", "Dock", ModeId::Dock),
        ("panel", "Panel", ModeId::Panel),
        ("tiling", "Tiling", ModeId::Tiling),
        ("browser", "Browser", ModeId::Browser),
        ("focus", "Focus", ModeId::Focus),
        ("tablet", "Tablet", ModeId::Tablet),
    ]
    .into_iter()
    .map(|(id, name, mode)| ModeSummary {
        id,
        name,
        mode,
        default_window_state: state_for_mode(mode),
    })
    .collect()
}
