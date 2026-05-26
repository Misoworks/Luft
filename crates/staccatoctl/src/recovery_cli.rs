use staccato_config::{list_config_backups, restore_latest_config_backup};
use staccato_ipc::{IpcRequest, send_request};

pub fn list_recovery_backups(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let backups = list_config_backups()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&backups)?);
    } else if backups.is_empty() {
        println!("No config backups");
    } else {
        for backup in backups {
            println!("{}", backup.display());
        }
    }
    Ok(())
}

pub fn rollback_config(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let backup = restore_latest_config_backup()?;
    reload_if_live();
    if json {
        println!(
            "{}",
            serde_json::json!({
                "restored": backup.is_some(),
                "backup": backup,
            })
        );
    } else if let Some(backup) = backup {
        println!("Restored {}", backup.display());
    } else {
        println!("No config backups");
    }
    Ok(())
}

fn reload_if_live() {
    let _ = send_request(&IpcRequest::Reload);
}
