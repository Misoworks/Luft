use luft_config::LuftConfig;
use luft_ipc::{LayoutEngine, Workspace, WorkspaceId};

pub fn layout_from_config(config: &LuftConfig) -> LayoutEngine {
    let mut workspaces = configured_workspaces(config);
    if workspaces.is_empty() {
        let count = config.workspaces.count.max(1);
        workspaces.extend((1..=count).map(|index| {
            let id = index.to_string();
            Workspace::empty(id.clone(), format!("Workspace {id}"))
        }));
    }
    let active = workspaces
        .first()
        .map(|workspace| workspace.id.clone())
        .unwrap_or_else(|| WorkspaceId("1".to_string()));

    LayoutEngine::new(workspaces, active)
        .unwrap_or_else(|_| LayoutEngine::with_default_workspaces())
}

fn configured_workspaces(config: &LuftConfig) -> Vec<Workspace> {
    config
        .workspaces
        .entries
        .iter()
        .map(|(id, workspace)| Workspace::empty(id.clone(), workspace.name.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use luft_config::WorkspaceConfig;

    #[test]
    fn default_config_materializes_runtime_workspace() {
        let config = LuftConfig::default();

        let engine = layout_from_config(&config);
        let workspaces = engine.workspaces().collect::<Vec<_>>();

        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].id, WorkspaceId("1".to_string()));
    }

    #[test]
    fn configured_workspace_entries_are_preserved() {
        let mut config = LuftConfig::default();
        config
            .workspaces
            .entries
            .insert("dev".to_string(), WorkspaceConfig::new("Dev"));

        let engine = layout_from_config(&config);
        let workspaces = engine.workspaces().collect::<Vec<_>>();

        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].id, WorkspaceId("dev".to_string()));
        assert_eq!(workspaces[0].name, "Dev");
    }
}
