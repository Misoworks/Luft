use super::notification_metadata::{
    action_pairs, clean_app_name, clean_icon_name, current_unix_time, strip_markup,
    urgency_from_hints,
};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Duration, Instant},
};
use tracing::{debug, warn};
use zbus::{
    blocking::connection, fdo, interface, object_server::SignalEmitter, zvariant::OwnedValue,
};

const SERVICE: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";
const INTERFACE: &str = "org.freedesktop.Notifications";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
const WORKER_TICK: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Default)]
pub struct NotificationSnapshot {
    pub items: Vec<NotificationItem>,
    pub toast_items: Vec<NotificationItem>,
    pub do_not_disturb: bool,
}

#[derive(Debug, Clone)]
pub struct NotificationItem {
    pub id: u32,
    pub app_name: String,
    pub app_icon: Option<String>,
    pub received_at: u64,
    pub summary: String,
    pub body: String,
    pub urgency: NotificationUrgency,
    pub actions: Vec<NotificationAction>,
}

#[derive(Debug, Clone)]
pub struct NotificationAction {
    pub key: String,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationUrgency {
    Low,
    Normal,
    Critical,
}

#[derive(Debug)]
pub struct NotificationService {
    snapshot: NotificationSnapshot,
    updates: Receiver<NotificationSnapshot>,
    commands: Sender<NotificationCommand>,
}

impl NotificationService {
    pub fn start() -> Self {
        let (updates_tx, updates_rx) = mpsc::channel();
        let (commands_tx, commands_rx) = mpsc::channel();

        thread::Builder::new()
            .name("staccato-notificationd".to_string())
            .spawn(move || {
                if let Err(error) = run_notification_worker(updates_tx, commands_rx) {
                    warn!(%error, "desktop notifications disabled");
                }
            })
            .ok();

        Self {
            snapshot: NotificationSnapshot::default(),
            updates: updates_rx,
            commands: commands_tx,
        }
    }

    pub fn refresh(&mut self) -> bool {
        let mut changed = false;
        while let Ok(snapshot) = self.updates.try_recv() {
            self.snapshot = snapshot;
            changed = true;
        }
        changed
    }

    pub fn snapshot(&self) -> &NotificationSnapshot {
        &self.snapshot
    }

    pub fn close(&mut self, id: u32) {
        self.snapshot.items.retain(|item| item.id != id);
        self.snapshot.toast_items.retain(|item| item.id != id);
        let _ = self.commands.send(NotificationCommand::Close(id));
    }

    pub fn clear_all(&mut self) {
        self.snapshot.items.clear();
        self.snapshot.toast_items.clear();
        let _ = self.commands.send(NotificationCommand::ClearAll);
    }

    pub fn invoke(&mut self, id: u32, action_key: String) {
        self.snapshot.items.retain(|item| item.id != id);
        self.snapshot.toast_items.retain(|item| item.id != id);
        let _ = self
            .commands
            .send(NotificationCommand::Invoke { id, action_key });
    }

    pub fn set_do_not_disturb(&mut self, enabled: bool) {
        self.snapshot.do_not_disturb = enabled;
        let _ = self
            .commands
            .send(NotificationCommand::SetDoNotDisturb(enabled));
    }
}

#[derive(Debug)]
enum NotificationCommand {
    Close(u32),
    ClearAll,
    SetDoNotDisturb(bool),
    Invoke { id: u32, action_key: String },
}

#[derive(Debug)]
struct NotificationState {
    next_id: u32,
    do_not_disturb: bool,
    items: Vec<StoredNotification>,
}

#[derive(Debug, Clone)]
struct StoredNotification {
    item: NotificationItem,
    toast_until: Option<Instant>,
    toast_visible: bool,
}

#[derive(Clone)]
struct NotificationShared {
    state: Arc<Mutex<NotificationState>>,
    changed: Sender<()>,
}

struct NotificationServer {
    shared: NotificationShared,
}

type Hints = HashMap<String, OwnedValue>;

#[interface(name = "org.freedesktop.Notifications")]
impl NotificationServer {
    fn get_capabilities(&self) -> Vec<String> {
        ["actions", "body", "icon-static"]
            .into_iter()
            .map(ToString::to_string)
            .collect()
    }

    fn get_server_information(&self) -> (String, String, String, String) {
        (
            "Staccato".to_string(),
            "Staccato".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
            "1.3".to_string(),
        )
    }

    fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: Hints,
        expire_timeout: i32,
    ) -> u32 {
        self.shared.upsert(
            app_name,
            replaces_id,
            app_icon,
            summary,
            body,
            actions,
            hints,
            expire_timeout,
        )
    }

    async fn close_notification(
        &self,
        id: u32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        if self.shared.remove(id) {
            let _ = self.shared.changed.send(());
            emitter.notification_closed(id, 3).await?;
            Ok(())
        } else {
            Err(fdo::Error::Failed("notification not found".to_string()))
        }
    }

    #[zbus(signal)]
    async fn notification_closed(
        emitter: &SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn action_invoked(
        emitter: &SignalEmitter<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;
}

impl NotificationShared {
    fn upsert(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: Hints,
        expire_timeout: i32,
    ) -> u32 {
        let mut state = self.state.lock().expect("notification state poisoned");
        let id = if replaces_id > 0 {
            state.next_id = state.next_id.max(replaces_id.saturating_add(1));
            replaces_id
        } else {
            let id = state.next_id.max(1);
            state.next_id = id.saturating_add(1).max(1);
            id
        };
        let urgency = urgency_from_hints(&hints);
        if state.do_not_disturb && urgency != NotificationUrgency::Critical {
            remove_stored(&mut state.items, id);
            let _ = self.changed.send(());
            return id;
        }
        let item = NotificationItem {
            id,
            app_name: clean_app_name(&app_name),
            app_icon: clean_icon_name(&app_icon),
            received_at: current_unix_time(),
            summary: strip_markup(&summary),
            body: strip_markup(&body),
            urgency,
            actions: action_pairs(actions),
        };
        let stored = StoredNotification {
            item,
            toast_until: expiration_for(expire_timeout, urgency),
            toast_visible: true,
        };

        if let Some(existing) = state
            .items
            .iter_mut()
            .find(|notification| notification.item.id == id)
        {
            *existing = stored;
        } else {
            state.items.insert(0, stored);
        }
        state.items.truncate(5);
        let _ = self.changed.send(());
        id
    }

    fn remove(&self, id: u32) -> bool {
        let mut state = self.state.lock().expect("notification state poisoned");
        let Some(index) = state
            .items
            .iter()
            .position(|notification| notification.item.id == id)
        else {
            return false;
        };
        state.items.remove(index);
        true
    }

    fn snapshot(&self) -> NotificationSnapshot {
        let now = Instant::now();
        let state = self.state.lock().expect("notification state poisoned");
        NotificationSnapshot {
            do_not_disturb: state.do_not_disturb,
            items: state
                .items
                .iter()
                .map(|notification| notification.item.clone())
                .collect(),
            toast_items: state
                .items
                .iter()
                .filter(|notification| notification.toast_visible)
                .filter(|notification| {
                    notification
                        .toast_until
                        .is_none_or(|expires_at| expires_at > now)
                })
                .map(|notification| notification.item.clone())
                .collect(),
        }
    }

    fn expire_toasts(&self) -> bool {
        let now = Instant::now();
        let mut state = self.state.lock().expect("notification state poisoned");
        let mut changed = false;
        for notification in &mut state.items {
            if notification.toast_visible
                && notification
                    .toast_until
                    .is_some_and(|expires_at| expires_at <= now)
            {
                notification.toast_visible = false;
                changed = true;
            }
        }
        changed
    }
}

fn run_notification_worker(
    updates: Sender<NotificationSnapshot>,
    commands: Receiver<NotificationCommand>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (changed_tx, changed_rx) = mpsc::channel();
    let shared = NotificationShared {
        state: Arc::new(Mutex::new(NotificationState {
            next_id: 1,
            do_not_disturb: false,
            items: Vec::new(),
        })),
        changed: changed_tx,
    };
    let connection = connection::Builder::session()?
        .name(SERVICE)?
        .serve_at(
            PATH,
            NotificationServer {
                shared: shared.clone(),
            },
        )?
        .build()?;

    debug!("desktop notification server ready");
    let _ = updates.send(shared.snapshot());
    loop {
        let mut dirty = false;
        while let Ok(command) = commands.try_recv() {
            dirty |= handle_command(&connection, &shared, command);
        }
        dirty |= shared.expire_toasts();
        dirty |= changed_rx.recv_timeout(WORKER_TICK).is_ok();
        if dirty {
            let _ = updates.send(shared.snapshot());
        }
    }
}

fn handle_command(
    connection: &zbus::blocking::Connection,
    shared: &NotificationShared,
    command: NotificationCommand,
) -> bool {
    match command {
        NotificationCommand::Close(id) => {
            if shared.remove(id) {
                emit_closed(connection, id, 2);
                return true;
            }
            false
        }
        NotificationCommand::ClearAll => {
            let ids = {
                let mut state = shared.state.lock().expect("notification state poisoned");
                let ids = state
                    .items
                    .iter()
                    .map(|notification| notification.item.id)
                    .collect::<Vec<_>>();
                state.items.clear();
                ids
            };
            for id in ids {
                emit_closed(connection, id, 2);
            }
            true
        }
        NotificationCommand::SetDoNotDisturb(enabled) => {
            {
                let mut state = shared.state.lock().expect("notification state poisoned");
                state.do_not_disturb = enabled;
            }
            true
        }
        NotificationCommand::Invoke { id, action_key } => {
            emit_action(connection, id, &action_key);
            if shared.remove(id) {
                emit_closed(connection, id, 2);
                return true;
            }
            false
        }
    }
}

fn remove_stored(items: &mut Vec<StoredNotification>, id: u32) {
    if let Some(index) = items
        .iter()
        .position(|notification| notification.item.id == id)
    {
        items.remove(index);
    }
}

fn emit_closed(connection: &zbus::blocking::Connection, id: u32, reason: u32) {
    let _ = connection.emit_signal::<&str, _, _, _, _>(
        None,
        PATH,
        INTERFACE,
        "NotificationClosed",
        &(id, reason),
    );
}

fn emit_action(connection: &zbus::blocking::Connection, id: u32, action_key: &str) {
    let _ = connection.emit_signal::<&str, _, _, _, _>(
        None,
        PATH,
        INTERFACE,
        "ActionInvoked",
        &(id, action_key),
    );
}

fn expiration_for(timeout: i32, urgency: NotificationUrgency) -> Option<Instant> {
    if timeout == 0 || urgency == NotificationUrgency::Critical {
        return None;
    }

    let timeout = if timeout < 0 {
        DEFAULT_TIMEOUT
    } else {
        Duration::from_millis(timeout as u64)
    };
    Some(Instant::now() + timeout)
}
