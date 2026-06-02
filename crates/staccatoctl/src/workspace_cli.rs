use staccato_config::{StaccatoConfig, load_config};
use staccato_ipc::{IpcRequest, IpcResponse, ProfileSummary, WorkspaceSummary, send_request};
use staccato_layout::{ProfileId, WorkspaceId, mode_for_profile};
use std::cmp::Ordering;

pub fn list_workspaces(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let workspaces = match send_request(&IpcRequest::ListWorkspaces) {
        Ok(IpcResponse::Workspaces { workspaces }) => workspaces,
        Ok(IpcResponse::Error { message }) => return Err(message.into()),
        Ok(response) => return Err(format!("unexpected response: {response:?}").into()),
        Err(_) => fallback_workspaces(&load_config()?.config),
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&workspaces)?);
    } else {
        for workspace in workspaces {
            println!(
                "{}\t{}\t{}\t{:?}",
                workspace.id.0, workspace.name, workspace.profile.0, workspace.mode
            );
        }
    }

    Ok(())
}

pub fn list_profiles(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let profiles = match send_request(&IpcRequest::ListProfiles) {
        Ok(IpcResponse::Profiles { profiles }) => profiles,
        Ok(IpcResponse::Error { message }) => return Err(message.into()),
        Ok(response) => return Err(format!("unexpected response: {response:?}").into()),
        Err(_) => fallback_profiles(&load_config()?.config),
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&profiles)?);
    } else {
        for profile in profiles {
            println!("{}\t{}\t{:?}", profile.id.0, profile.name, profile.mode);
        }
    }

    Ok(())
}

fn fallback_workspaces(config: &StaccatoConfig) -> Vec<WorkspaceSummary> {
    if config.workspaces.entries.is_empty() {
        let count = config.workspaces.count.max(1);
        return (1..=count)
            .map(|index| {
                workspace_summary(
                    index.to_string(),
                    format!("Workspace {index}"),
                    &config.general.default_profile,
                )
            })
            .collect();
    }

    let mut entries = config.workspaces.entries.iter().collect::<Vec<_>>();
    entries.sort_by(|(left, _), (right, _)| compare_ids(left, right));
    entries
        .into_iter()
        .map(|(id, workspace)| {
            workspace_summary(id.clone(), workspace.name.clone(), &workspace.profile)
        })
        .collect()
}

fn fallback_profiles(config: &StaccatoConfig) -> Vec<ProfileSummary> {
    let mut profiles = config
        .workspaces
        .entries
        .values()
        .map(|workspace| workspace.profile.clone())
        .chain(std::iter::once(config.general.default_profile.clone()))
        .chain(["panel-default".to_string(), "dock-default".to_string()])
        .collect::<Vec<_>>();
    profiles.sort();
    profiles.dedup();
    profiles
        .into_iter()
        .map(|profile| {
            let id = ProfileId(profile);
            ProfileSummary {
                name: profile_name(&id),
                mode: mode_for_profile(&id),
                id,
            }
        })
        .collect()
}

fn workspace_summary(id: String, name: String, profile: &str) -> WorkspaceSummary {
    let profile = ProfileId(profile.to_string());
    WorkspaceSummary {
        id: WorkspaceId(id),
        name,
        mode: mode_for_profile(&profile),
        profile,
    }
}

fn compare_ids(left: &str, right: &str) -> Ordering {
    match (left.parse::<u32>(), right.parse::<u32>()) {
        (Ok(left), Ok(right)) => left.cmp(&right),
        (Ok(_), Err(_)) => Ordering::Less,
        (Err(_), Ok(_)) => Ordering::Greater,
        (Err(_), Err(_)) => left.cmp(right),
    }
}

fn profile_name(profile: &ProfileId) -> String {
    profile
        .0
        .split('-')
        .map(title_case)
        .collect::<Vec<_>>()
        .join(" ")
}

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
