use std::collections::HashMap;

use zbus::{fdo, interface};
use zvariant::{OwnedValue, Value};

const APPEARANCE: &str = "org.freedesktop.appearance";
const GNOME_WM: &str = "org.gnome.desktop.wm.preferences";
const GNOME_INTERFACE: &str = "org.gnome.desktop.interface";

pub struct PortalSettings {
    values: HashMap<String, HashMap<String, OwnedValue>>,
}

impl PortalSettings {
    pub fn new() -> Self {
        let mut appearance = HashMap::new();
        appearance.insert("color-scheme".to_string(), OwnedValue::from(1u32));
        appearance.insert("contrast".to_string(), OwnedValue::from(0u32));
        appearance.insert("reduced-motion".to_string(), OwnedValue::from(0u32));

        let mut wm_preferences = HashMap::new();
        wm_preferences.insert(
            "button-layout".to_string(),
            Value::from(":minimize,:maximize,:close")
                .try_into()
                .expect("portal string value"),
        );

        let mut interface = HashMap::new();
        interface.insert(
            "gtk-decoration-layout".to_string(),
            Value::from(":minimize,:maximize,:close")
                .try_into()
                .expect("portal string value"),
        );
        interface.insert(
            "enable-animations".to_string(),
            OwnedValue::from(true),
        );

        let mut values = HashMap::new();
        values.insert(APPEARANCE.to_string(), appearance);
        values.insert(GNOME_WM.to_string(), wm_preferences);
        values.insert(GNOME_INTERFACE.to_string(), interface);
        Self { values }
    }

    fn lookup(&self, namespace: &str, key: &str) -> Option<OwnedValue> {
        self.values.get(namespace)?.get(key).cloned()
    }

    fn namespace_matches(filter: &str, namespace: &str) -> bool {
        if filter.is_empty() {
            return true;
        }
        if let Some(prefix) = filter.strip_suffix(".*") {
            return namespace.starts_with(prefix);
        }
        filter == namespace
    }

    fn filtered(&self, namespaces: &[String]) -> HashMap<String, HashMap<String, OwnedValue>> {
        if namespaces.is_empty() || namespaces.iter().any(String::is_empty) {
            return self.values.clone();
        }

        self.values
            .iter()
            .filter(|(namespace, _)| {
                namespaces
                    .iter()
                    .any(|filter| Self::namespace_matches(filter, namespace))
            })
            .map(|(namespace, keys)| (namespace.clone(), keys.clone()))
            .collect()
    }
}

#[interface(name = "org.freedesktop.impl.portal.Settings")]
impl PortalSettings {
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        1
    }

    fn read(&self, namespace: &str, key: &str) -> fdo::Result<OwnedValue> {
        self.lookup(namespace, key)
            .ok_or_else(|| fdo::Error::InvalidArgs(format!("unknown setting {namespace}::{key}")))
    }

    fn read_all(
        &self,
        namespaces: Vec<String>,
    ) -> fdo::Result<HashMap<String, HashMap<String, Value<'_>>>> {
        Ok(self
            .filtered(&namespaces)
            .into_iter()
            .map(|(namespace, keys)| {
                (
                    namespace,
                    keys.into_iter()
                        .map(|(key, value)| (key, value.into()))
                        .collect(),
                )
            })
            .collect())
    }
}
