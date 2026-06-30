use asher_config::AsherConfig;
use asher_ipc::{LayoutEngine, Workspace, WorkspaceId};

pub fn layout_from_config(config: &AsherConfig) -> LayoutEngine {
    let mut workspaces = configured_workspaces(config);
    if workspaces.is_empty() {
        let count = config.workspaces.count.max(1);
        workspaces.extend((1..=count).map(|index| {
            let id = index.to_string();
            Workspace::empty(
                id.clone(),
                format!("Workspace {id}"),
                config.general.default_profile.clone(),
            )
        }));
    }
    let active = workspaces
        .first()
        .map(|workspace| workspace.id.clone())
        .unwrap_or_else(|| WorkspaceId("1".to_string()));

    LayoutEngine::new(workspaces, active)
        .unwrap_or_else(|_| LayoutEngine::with_default_workspaces())
}

fn configured_workspaces(config: &AsherConfig) -> Vec<Workspace> {
    config
        .workspaces
        .entries
        .iter()
        .map(|(id, workspace)| {
            Workspace::empty(
                id.clone(),
                workspace.name.clone(),
                workspace.profile.clone(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use asher_config::WorkspaceConfig;
    use asher_ipc::ProfileId;

    #[test]
    fn default_config_materializes_runtime_workspace() {
        let mut config = AsherConfig::default();
        config.general.default_profile = "panel-default".to_string();

        let engine = layout_from_config(&config);
        let workspaces = engine.workspaces().collect::<Vec<_>>();

        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].id, WorkspaceId("1".to_string()));
        assert_eq!(
            workspaces[0].profile_id,
            ProfileId("panel-default".to_string())
        );
    }

    #[test]
    fn configured_workspace_entries_are_preserved() {
        let mut config = AsherConfig::default();
        config.workspaces.entries.insert(
            "dev".to_string(),
            WorkspaceConfig::new("Dev", "browser-dev"),
        );

        let engine = layout_from_config(&config);
        let workspaces = engine.workspaces().collect::<Vec<_>>();

        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].id, WorkspaceId("dev".to_string()));
        assert_eq!(workspaces[0].name, "Dev");
        assert_eq!(
            workspaces[0].profile_id,
            ProfileId("browser-dev".to_string())
        );
    }
}
