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
}

#[derive(Debug, Clone)]
pub struct NotificationItem {
    pub id: u32,
    pub app_name: String,
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
        let _ = self.commands.send(NotificationCommand::Close(id));
    }

    pub fn invoke(&mut self, id: u32, action_key: String) {
        self.snapshot.items.retain(|item| item.id != id);
        let _ = self
            .commands
            .send(NotificationCommand::Invoke { id, action_key });
    }
}

#[derive(Debug)]
enum NotificationCommand {
    Close(u32),
    Invoke { id: u32, action_key: String },
}

#[derive(Debug)]
struct NotificationState {
    next_id: u32,
    items: Vec<StoredNotification>,
}

#[derive(Debug, Clone)]
struct StoredNotification {
    item: NotificationItem,
    expires_at: Option<Instant>,
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
        _app_icon: String,
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
        let item = NotificationItem {
            id,
            app_name,
            summary: strip_markup(&summary),
            body: strip_markup(&body),
            urgency,
            actions: action_pairs(actions),
        };
        let stored = StoredNotification {
            item,
            expires_at: expiration_for(expire_timeout, urgency),
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
        let state = self.state.lock().expect("notification state poisoned");
        NotificationSnapshot {
            items: state
                .items
                .iter()
                .map(|notification| notification.item.clone())
                .collect(),
        }
    }

    fn expire_due(&self) -> Vec<u32> {
        let now = Instant::now();
        let mut state = self.state.lock().expect("notification state poisoned");
        let mut expired = Vec::new();
        state.items.retain(|notification| {
            let keep = notification
                .expires_at
                .is_none_or(|expires_at| expires_at > now);
            if !keep {
                expired.push(notification.item.id);
            }
            keep
        });
        expired
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
        for id in shared.expire_due() {
            emit_closed(&connection, id, 1);
            dirty = true;
        }
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

fn urgency_from_hints(hints: &Hints) -> NotificationUrgency {
    match hints
        .get("urgency")
        .and_then(|value| u8::try_from(value.clone()).ok())
    {
        Some(0) => NotificationUrgency::Low,
        Some(2) => NotificationUrgency::Critical,
        _ => NotificationUrgency::Normal,
    }
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

fn action_pairs(actions: Vec<String>) -> Vec<NotificationAction> {
    actions
        .chunks(2)
        .filter_map(|pair| {
            let key = pair.first()?.trim();
            let label = pair.get(1)?.trim();
            (!key.is_empty() && !label.is_empty()).then(|| NotificationAction {
                key: key.to_string(),
                label: strip_markup(label),
            })
        })
        .collect()
}

fn strip_markup(text: &str) -> String {
    let mut output = String::new();
    let mut inside_tag = false;
    for character in text.chars() {
        match character {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => output.push(character),
            _ => {}
        }
    }
    output
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}
