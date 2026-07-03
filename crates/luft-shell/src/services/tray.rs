use std::{
    io::Cursor,
    process,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Duration, Instant},
};
use tracing::{debug, warn};
use zbus::{
    blocking::{Proxy, connection},
    fdo, interface,
    message::Header,
    object_server::SignalEmitter,
};

const WATCHER_SERVICE: &str = "org.kde.StatusNotifierWatcher";
const WATCHER_PATH: &str = "/StatusNotifierWatcher";
const WATCHER_INTERFACE: &str = "org.kde.StatusNotifierWatcher";
const ITEM_PATH: &str = "/StatusNotifierItem";
const ITEM_INTERFACES: [&str; 2] = [
    "org.kde.StatusNotifierItem",
    "org.freedesktop.StatusNotifierItem",
];
const REFRESH_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Default)]
pub struct TraySnapshot {
    pub items: Vec<TrayItem>,
}

#[derive(Debug, Clone)]
pub struct TrayItem {
    pub registration: TrayRegistration,
    pub title: String,
    pub icon_name: Option<String>,
    pub icon_pixmap_uri: Option<String>,
    pub status: TrayItemStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayRegistration {
    pub raw: String,
    pub service: String,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayItemStatus {
    Passive,
    Active,
    NeedsAttention,
}

#[derive(Debug)]
pub struct TrayService {
    snapshot: TraySnapshot,
    updates: Receiver<TraySnapshot>,
    commands: Sender<TrayCommand>,
}

impl TrayService {
    pub fn start() -> Self {
        let (updates_tx, updates_rx) = mpsc::channel();
        let (commands_tx, commands_rx) = mpsc::channel();

        thread::Builder::new()
            .name("luft-status-notifier".to_string())
            .spawn(move || {
                if let Err(error) = run_tray_worker(updates_tx, commands_rx) {
                    warn!(%error, "status notifier tray disabled");
                }
            })
            .ok();

        Self {
            snapshot: TraySnapshot::default(),
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

    pub fn snapshot(&self) -> &TraySnapshot {
        &self.snapshot
    }

    pub fn activate(&self, item: &TrayItem, x: i32, y: i32) {
        let _ = self.commands.send(TrayCommand::Activate {
            registration: item.registration.clone(),
            x,
            y,
        });
    }

    pub fn context_menu(&self, item: &TrayItem, x: i32, y: i32) {
        let _ = self.commands.send(TrayCommand::ContextMenu {
            registration: item.registration.clone(),
            x,
            y,
        });
    }
}

#[derive(Debug)]
enum TrayCommand {
    Activate {
        registration: TrayRegistration,
        x: i32,
        y: i32,
    },
    ContextMenu {
        registration: TrayRegistration,
        x: i32,
        y: i32,
    },
}

#[derive(Debug, Default)]
struct WatcherData {
    items: Vec<TrayRegistration>,
    host_registered: bool,
}

#[derive(Clone)]
struct WatcherShared {
    data: Arc<Mutex<WatcherData>>,
    refresh: Sender<()>,
}

struct StatusNotifierWatcher {
    shared: WatcherShared,
}

#[interface(name = "org.kde.StatusNotifierWatcher")]
impl StatusNotifierWatcher {
    async fn register_status_notifier_item(
        &self,
        service: String,
        #[zbus(header)] header: Header<'_>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        let sender = header.sender().map(ToString::to_string);
        let Some(registration) =
            TrayRegistration::from_service_argument(&service, sender.as_deref())
        else {
            return Ok(());
        };

        let raw = registration.raw.clone();
        if self.shared.add_item(registration) {
            let _ = self.shared.refresh.send(());
            emitter.status_notifier_item_registered(&raw).await?;
        }
        Ok(())
    }

    async fn register_status_notifier_host(
        &self,
        service: String,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        if self.shared.register_host(service) {
            let _ = self.shared.refresh.send(());
            emitter.status_notifier_host_registered().await?;
        }
        Ok(())
    }

    #[zbus(property, name = "RegisteredStatusNotifierItems")]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.shared.registered_items()
    }

    #[zbus(property, name = "IsStatusNotifierHostRegistered")]
    fn is_status_notifier_host_registered(&self) -> bool {
        self.shared.host_registered()
    }

    #[zbus(property, name = "ProtocolVersion")]
    fn protocol_version(&self) -> i32 {
        0
    }

    #[zbus(signal)]
    async fn status_notifier_item_registered(
        emitter: &SignalEmitter<'_>,
        service: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn status_notifier_item_unregistered(
        emitter: &SignalEmitter<'_>,
        service: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn status_notifier_host_registered(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

impl WatcherShared {
    fn add_item(&self, registration: TrayRegistration) -> bool {
        let mut data = self.data.lock().expect("tray watcher state poisoned");
        if data
            .items
            .iter()
            .any(|item| item.key() == registration.key())
        {
            return false;
        }

        data.items.push(registration);
        true
    }

    fn register_host(&self, service: String) -> bool {
        let mut data = self.data.lock().expect("tray watcher state poisoned");
        let was_registered = data.host_registered;
        data.host_registered = !service.trim().is_empty();
        data.host_registered && !was_registered
    }

    fn registered_items(&self) -> Vec<String> {
        self.data
            .lock()
            .expect("tray watcher state poisoned")
            .items
            .iter()
            .map(|item| item.raw.clone())
            .collect()
    }

    fn host_registered(&self) -> bool {
        self.data
            .lock()
            .expect("tray watcher state poisoned")
            .host_registered
    }

    fn items(&self) -> Vec<TrayRegistration> {
        self.data
            .lock()
            .expect("tray watcher state poisoned")
            .items
            .clone()
    }

    fn prune_disconnected(&self, connection: &zbus::blocking::Connection) -> Vec<String> {
        let mut data = self.data.lock().expect("tray watcher state poisoned");
        let mut removed = Vec::new();
        data.items.retain(|item| {
            let connected = bus_name_has_owner(connection, &item.service);
            if !connected {
                removed.push(item.raw.clone());
            }
            connected
        });
        removed
    }
}

impl TrayRegistration {
    fn from_service_argument(service: &str, sender: Option<&str>) -> Option<Self> {
        let service = service.trim();
        if service.is_empty() {
            return None;
        }

        if service.starts_with('/') {
            let sender = sender?;
            return Some(Self::new(
                format!("{sender}{service}"),
                sender.to_string(),
                service.to_string(),
            ));
        }

        if let Some((bus, path)) = service.split_once('/') {
            return Some(Self::new(
                service.to_string(),
                bus.to_string(),
                format!("/{path}"),
            ));
        }

        Some(Self::new(
            service.to_string(),
            service.to_string(),
            ITEM_PATH.to_string(),
        ))
    }

    fn new(raw: String, service: String, path: String) -> Self {
        Self { raw, service, path }
    }

    fn key(&self) -> String {
        format!("{}{}", self.service, self.path)
    }
}

fn run_tray_worker(
    updates: Sender<TraySnapshot>,
    commands: Receiver<TrayCommand>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (refresh_tx, refresh_rx) = mpsc::channel();
    let shared = WatcherShared {
        data: Arc::new(Mutex::new(WatcherData::default())),
        refresh: refresh_tx,
    };
    let watcher = StatusNotifierWatcher {
        shared: shared.clone(),
    };
    let host_name = format!("org.freedesktop.StatusNotifierHost-{}", process::id());
    let connection = connection::Builder::session()?
        .name(WATCHER_SERVICE)?
        .serve_at(WATCHER_PATH, watcher)?
        .build()?;

    connection.request_name(host_name.as_str())?;
    shared.register_host(host_name.clone());
    connection.emit_signal::<&str, _, _, _, _>(
        None,
        WATCHER_PATH,
        WATCHER_INTERFACE,
        "StatusNotifierHostRegistered",
        &(),
    )?;

    debug!(host = %host_name, "status notifier tray ready");
    publish_snapshot(&connection, &shared, &updates);
    let mut last_refresh = Instant::now();

    loop {
        while let Ok(command) = commands.try_recv() {
            handle_command(&connection, command);
        }

        let timed_refresh = last_refresh.elapsed() >= REFRESH_INTERVAL;
        let signalled_refresh = refresh_rx.recv_timeout(Duration::from_millis(200)).is_ok();

        if timed_refresh || signalled_refresh {
            publish_snapshot(&connection, &shared, &updates);
            last_refresh = Instant::now();
        }
    }
}

fn publish_snapshot(
    connection: &zbus::blocking::Connection,
    shared: &WatcherShared,
    updates: &Sender<TraySnapshot>,
) {
    for raw in shared.prune_disconnected(connection) {
        let _ = connection.emit_signal::<&str, _, _, _, _>(
            None,
            WATCHER_PATH,
            WATCHER_INTERFACE,
            "StatusNotifierItemUnregistered",
            &(raw.as_str(),),
        );
    }

    let items = shared
        .items()
        .iter()
        .filter_map(|registration| read_tray_item(connection, registration))
        .collect();
    let _ = updates.send(TraySnapshot { items });
}

fn read_tray_item(
    connection: &zbus::blocking::Connection,
    registration: &TrayRegistration,
) -> Option<TrayItem> {
    for interface in ITEM_INTERFACES {
        let Ok(proxy) = Proxy::new(
            connection,
            registration.service.as_str(),
            registration.path.as_str(),
            interface,
        ) else {
            continue;
        };
        let status = proxy
            .get_property::<String>("Status")
            .map(|status| TrayItemStatus::from_str(&status))
            .unwrap_or(TrayItemStatus::Active);
        let title = proxy
            .get_property::<String>("Title")
            .ok()
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| registration.service.clone());
        let icon_name = tray_icon_name(&proxy, status);
        let icon_pixmap_uri = tray_icon_pixmap_uri(&proxy, status);

        return Some(TrayItem {
            registration: registration.clone(),
            title,
            icon_name,
            icon_pixmap_uri,
            status,
        });
    }

    None
}

fn bus_name_has_owner(connection: &zbus::blocking::Connection, service: &str) -> bool {
    let Ok(proxy) = Proxy::new(
        connection,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    ) else {
        return true;
    };

    proxy
        .call::<_, _, bool>("NameHasOwner", &(service,))
        .unwrap_or(true)
}

fn tray_icon_name(proxy: &Proxy<'_>, status: TrayItemStatus) -> Option<String> {
    let preferred = if status == TrayItemStatus::NeedsAttention {
        ["AttentionIconName", "IconName"]
    } else {
        ["IconName", "AttentionIconName"]
    };

    preferred.into_iter().find_map(|property| {
        proxy
            .get_property::<String>(property)
            .ok()
            .filter(|name| !name.trim().is_empty())
    })
}

fn tray_icon_pixmap_uri(proxy: &Proxy<'_>, status: TrayItemStatus) -> Option<String> {
    let preferred = if status == TrayItemStatus::NeedsAttention {
        ["AttentionIconPixmap", "IconPixmap"]
    } else {
        ["IconPixmap", "AttentionIconPixmap"]
    };

    preferred.into_iter().find_map(|property| {
        proxy
            .get_property::<Vec<(i32, i32, Vec<u8>)>>(property)
            .ok()
            .and_then(best_icon_pixmap_uri)
    })
}

fn best_icon_pixmap_uri(pixmaps: Vec<(i32, i32, Vec<u8>)>) -> Option<String> {
    pixmaps
        .into_iter()
        .filter(|(width, height, pixels)| {
            *width > 0 && *height > 0 && pixels.len() == (*width as usize) * (*height as usize) * 4
        })
        .max_by_key(|(width, height, _)| width * height)
        .and_then(|(width, height, pixels)| icon_pixmap_uri(width, height, &pixels))
}

fn icon_pixmap_uri(width: i32, height: i32, argb: &[u8]) -> Option<String> {
    use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};

    let mut rgba = Vec::with_capacity(argb.len());
    for pixel in argb.chunks_exact(4) {
        rgba.extend_from_slice(&[pixel[1], pixel[2], pixel[3], pixel[0]]);
    }

    let mut png = Cursor::new(Vec::new());
    PngEncoder::new(&mut png)
        .write_image(&rgba, width as u32, height as u32, ColorType::Rgba8.into())
        .ok()?;
    Some(crate::web::icons::bytes_data_uri(
        "image/png",
        png.get_ref(),
    ))
}

fn handle_command(connection: &zbus::blocking::Connection, command: TrayCommand) {
    match command {
        TrayCommand::Activate { registration, x, y } => {
            call_item_method(connection, &registration, "Activate", x, y);
        }
        TrayCommand::ContextMenu { registration, x, y } => {
            call_item_method(connection, &registration, "ContextMenu", x, y);
        }
    }
}

fn call_item_method(
    connection: &zbus::blocking::Connection,
    registration: &TrayRegistration,
    method: &str,
    x: i32,
    y: i32,
) {
    for interface in ITEM_INTERFACES {
        let Ok(proxy) = Proxy::new(
            connection,
            registration.service.as_str(),
            registration.path.as_str(),
            interface,
        ) else {
            continue;
        };
        if proxy.call::<_, _, ()>(method, &(x, y)).is_ok() {
            return;
        }
    }
}

impl TrayItemStatus {
    fn from_str(status: &str) -> Self {
        match status {
            "Passive" => Self::Passive,
            "NeedsAttention" => Self::NeedsAttention,
            _ => Self::Active,
        }
    }
}
