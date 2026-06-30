use super::*;

#[test]
fn registers_window_in_active_workspace() {
    let mut engine = LayoutEngine::with_default_workspaces();
    let id = engine
        .register_window(WindowInfo::new(
            WindowId(0),
            engine.active_workspace().clone(),
            crate::Rect::new(10, 20, 640, 480),
        ))
        .unwrap();

    let arrangement = engine.arrange_active().unwrap();
    assert_eq!(
        arrangement.windows.get(&id),
        Some(&crate::Rect::new(10, 20, 640, 480))
    );
}

#[test]
fn switches_and_moves_windows_between_workspaces() {
    let mut engine = LayoutEngine::with_default_workspaces();
    let first = engine.active_workspace().clone();
    let second = WorkspaceId("2".to_string());
    engine.ensure_workspace(
        second.clone(),
        "Workspace 2".to_string(),
        ProfileId("panel-default".to_string()),
    );
    let id = engine
        .register_window(WindowInfo::new(
            WindowId(0),
            first,
            crate::Rect::new(40, 40, 800, 500),
        ))
        .unwrap();

    engine.move_window_to_workspace(id, &second).unwrap();
    engine.switch_workspace(&second).unwrap();

    assert_eq!(engine.window(id).unwrap().workspace, second);
    assert!(engine.arrange_active().unwrap().windows.contains_key(&id));
}

#[test]
fn changes_workspace_profile_without_switching_workspace() {
    let mut engine = LayoutEngine::with_default_workspaces();
    let workspace = WorkspaceId("2".to_string());
    let profile = ProfileId("panel-writing".to_string());
    engine.ensure_workspace(
        workspace.clone(),
        "Workspace 2".to_string(),
        ProfileId("panel-default".to_string()),
    );

    engine
        .set_workspace_profile(&workspace, profile.clone())
        .unwrap();

    let updated = engine
        .workspaces()
        .find(|entry| entry.id == workspace)
        .unwrap();
    assert_eq!(updated.profile_id, profile);
    assert_eq!(engine.active_workspace(), &WorkspaceId("1".to_string()));
}

#[test]
fn relative_workspace_creates_next_workspace() {
    let mut engine = LayoutEngine::with_default_workspaces();
    engine
        .register_window(WindowInfo::new(
            WindowId(0),
            engine.active_workspace().clone(),
            crate::Rect::new(10, 20, 640, 480),
        ))
        .unwrap();

    let next = engine.relative_workspace(1).unwrap();

    assert_eq!(next, WorkspaceId("2".to_string()));
    assert!(
        engine
            .workspaces()
            .any(|workspace| workspace.id == next && workspace.name == "Workspace 2")
    );
}

#[test]
fn relative_workspace_does_not_create_past_empty_workspace() {
    let mut engine = LayoutEngine::with_default_workspaces();

    assert_eq!(engine.relative_workspace(1), None);
    assert_eq!(engine.workspaces().count(), 1);

    engine
        .register_window(WindowInfo::new(
            WindowId(0),
            engine.active_workspace().clone(),
            crate::Rect::new(10, 20, 640, 480),
        ))
        .unwrap();
    let empty = engine.relative_workspace(1).unwrap();
    engine.switch_workspace(&empty).unwrap();

    assert_eq!(engine.relative_workspace(1), None);
    assert_eq!(engine.workspaces().count(), 2);
}

#[test]
fn relative_workspace_uses_numeric_order_after_nine() {
    let mut engine = LayoutEngine::with_default_workspaces();
    for index in 2..=10 {
        let id = WorkspaceId(index.to_string());
        engine.ensure_workspace(
            id,
            format!("Workspace {index}"),
            ProfileId("panel-default".to_string()),
        );
    }
    let ninth = WorkspaceId("9".to_string());
    engine.switch_workspace(&ninth).unwrap();
    engine
        .register_window(WindowInfo::new(
            WindowId(0),
            ninth,
            crate::Rect::new(10, 20, 640, 480),
        ))
        .unwrap();

    assert_eq!(
        engine.relative_workspace(1),
        Some(WorkspaceId("10".to_string()))
    );
    assert_eq!(
        engine
            .workspaces()
            .map(|workspace| workspace.id.0.clone())
            .collect::<Vec<_>>(),
        (1..=10).map(|index| index.to_string()).collect::<Vec<_>>()
    );
}
