//! Appareils Bluetooth via `Windows.Devices.Enumeration` : deux observateurs
//! d'appareils, sans interrogation périodique.
//!
//! - Les appareils appairés (points de terminaison d'association) donnent le
//!   nom, l'état de connexion et la classe (audio ou non).
//! - Les nœuds PnP Bluetooth portent le niveau de batterie que Windows lit
//!   (profil mains libres, service batterie BLE) : `DEVPKEY_Bluetooth_Battery`.
//!
//! Les deux se rejoignent par leur conteneur (`ContainerId`). Les gestionnaires
//! d'événements mettent à jour un état partagé puis réveillent le thread, qui
//! publie la liste quand elle change.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::UnboundedSender;
use windows::Devices::Bluetooth::{BluetoothDevice, BluetoothLEDevice};
use windows::Devices::Enumeration::{
    DeviceInformation, DeviceInformationKind, DeviceInformationUpdate, DeviceWatcher,
};
use windows::Foundation::{IPropertyValue, PropertyType, TypedEventHandler};
use windows_collections::{IIterable, IMapView};
use windows_core::{HSTRING, IInspectable, Interface};

use crate::{Device, Wake};

const IS_CONNECTED: &str = "System.Devices.Aep.IsConnected";
const AEP_CONTAINER: &str = "System.Devices.Aep.ContainerId";
/// Classe Bluetooth classique (« major class ») : 4 = audio/vidéo.
const COD_MAJOR: &str = "System.Devices.Aep.Bluetooth.Cod.Major";
const CONTAINER: &str = "System.Devices.ContainerId";
/// `DEVPKEY_Bluetooth_Battery` : niveau en %, sur le nœud PnP.
const BATTERY: &str = "{104EA319-6EE2-4701-BD47-8DDBF425BBE5} 2";
const COD_AUDIO_VIDEO: u32 = 4;

#[derive(Default)]
struct Endpoint {
    name: String,
    container: Option<String>,
    connected: bool,
    audio: bool,
}

#[derive(Default)]
struct Node {
    container: Option<String>,
    level: Option<u8>,
}

#[derive(Default)]
struct State {
    endpoints: HashMap<String, Endpoint>,
    nodes: HashMap<String, Node>,
    /// Énumérations initiales terminées (appairés, nœuds PnP).
    ready: [bool; 2],
}

impl State {
    fn devices(&self) -> Vec<Device> {
        let mut devices: Vec<Device> = self
            .endpoints
            .iter()
            .map(|(id, e)| Device {
                id: id.clone(),
                name: e.name.clone(),
                connected: e.connected,
                level: e.container.as_ref().and_then(|c| {
                    self.nodes
                        .values()
                        .filter(|n| n.container.as_ref() == Some(c))
                        .filter_map(|n| n.level)
                        .max()
                }),
                audio: e.audio,
            })
            .collect();
        devices.sort_by(|a, b| a.id.cmp(&b.id));
        devices
    }
}

/// Thread dédié : observateurs, puis publication à chaque réveil jusqu'à
/// `Wake::Stop` (envoyé à l'arrêt du module).
pub fn spawn(tx: UnboundedSender<Vec<Device>>) -> Sender<Wake> {
    let (wake_tx, wake_rx) = channel();
    let handler_tx = wake_tx.clone();
    std::thread::spawn(move || {
        if let Err(e) = run(&tx, handler_tx, &wake_rx) {
            log::warn!("bluetooth : appareils indisponibles : {e}");
        }
    });
    wake_tx
}

fn run(
    tx: &UnboundedSender<Vec<Device>>,
    wake_tx: Sender<Wake>,
    wake: &Receiver<Wake>,
) -> windows::core::Result<()> {
    let state = Arc::new(Mutex::new(State::default()));

    let paired = format!(
        "({}) OR ({})",
        BluetoothDevice::GetDeviceSelectorFromPairingState(true)?,
        BluetoothLEDevice::GetDeviceSelectorFromPairingState(true)?
    );
    let endpoints = watcher(
        &paired,
        &[IS_CONNECTED, AEP_CONTAINER, COD_MAJOR],
        DeviceInformationKind::AssociationEndpoint,
    )?;
    let nodes = watcher(
        "System.Devices.DeviceInstanceId:~<\"BTH\"",
        &[BATTERY, CONTAINER],
        DeviceInformationKind::Device,
    )?;
    let subscriptions = [
        subscribe(&endpoints, 0, &state, &wake_tx)?,
        subscribe(&nodes, 1, &state, &wake_tx)?,
    ];
    endpoints.Start()?;
    nodes.Start()?;

    let mut last: Option<Vec<Device>> = None;
    while let Ok(first) = wake.recv() {
        if std::iter::once(first)
            .chain(wake.try_iter())
            .any(|w| w == Wake::Stop)
        {
            break;
        }
        let devices = {
            let state = state.lock().unwrap_or_else(|e| e.into_inner());
            if !state.ready.iter().all(|r| *r) {
                continue;
            }
            state.devices()
        };
        if last.as_ref() != Some(&devices) {
            last = Some(devices.clone());
            if tx.send(devices).is_err() {
                break;
            }
        }
    }

    for (w, tokens) in [(&endpoints, subscriptions[0]), (&nodes, subscriptions[1])] {
        let _ = w.Stop();
        let _ = w.RemoveAdded(tokens[0]);
        let _ = w.RemoveUpdated(tokens[1]);
        let _ = w.RemoveRemoved(tokens[2]);
        let _ = w.RemoveEnumerationCompleted(tokens[3]);
    }
    Ok(())
}

fn watcher(
    aqs: &str,
    properties: &[&str],
    kind: DeviceInformationKind,
) -> windows::core::Result<DeviceWatcher> {
    let properties: Vec<HSTRING> = properties.iter().map(|p| HSTRING::from(*p)).collect();
    DeviceInformation::CreateWatcherWithKindAqsFilterAndAdditionalProperties(
        &HSTRING::from(aqs),
        &IIterable::<HSTRING>::from(properties),
        kind,
    )
}

/// Branche les quatre événements de `watcher` sur l'état partagé.
/// `which` : 0 pour les appareils appairés, 1 pour les nœuds PnP.
fn subscribe(
    watcher: &DeviceWatcher,
    which: usize,
    state: &Arc<Mutex<State>>,
    wake: &Sender<Wake>,
) -> windows::core::Result<[i64; 4]> {
    let added = {
        let (state, wake) = (state.clone(), wake.clone());
        TypedEventHandler::<DeviceWatcher, DeviceInformation>::new(move |_, info| {
            if let Some(info) = info.as_ref() {
                let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                add(&mut s, which, info);
                drop(s);
                let _ = wake.send(Wake::Changed);
            }
            Ok(())
        })
    };
    let updated = {
        let (state, wake) = (state.clone(), wake.clone());
        TypedEventHandler::<DeviceWatcher, DeviceInformationUpdate>::new(move |_, update| {
            if let Some(update) = update.as_ref() {
                let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                apply(&mut s, which, update);
                drop(s);
                let _ = wake.send(Wake::Changed);
            }
            Ok(())
        })
    };
    let removed = {
        let (state, wake) = (state.clone(), wake.clone());
        TypedEventHandler::<DeviceWatcher, DeviceInformationUpdate>::new(move |_, update| {
            if let Some(id) = update.as_ref().and_then(|u| u.Id().ok()) {
                let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                let id = id.to_string();
                if which == 0 {
                    s.endpoints.remove(&id);
                } else {
                    s.nodes.remove(&id);
                }
                drop(s);
                let _ = wake.send(Wake::Changed);
            }
            Ok(())
        })
    };
    let completed = {
        let (state, wake) = (state.clone(), wake.clone());
        TypedEventHandler::<DeviceWatcher, IInspectable>::new(move |_, _| {
            state.lock().unwrap_or_else(|e| e.into_inner()).ready[which] = true;
            let _ = wake.send(Wake::Changed);
            Ok(())
        })
    };
    Ok([
        watcher.Added(&added)?,
        watcher.Updated(&updated)?,
        watcher.Removed(&removed)?,
        watcher.EnumerationCompleted(&completed)?,
    ])
}

fn add(state: &mut State, which: usize, info: &DeviceInformation) {
    let Ok(id) = info.Id() else { return };
    let Ok(props) = info.Properties() else { return };
    if which == 0 {
        let endpoint = Endpoint {
            name: info.Name().map(|n| n.to_string()).unwrap_or_default(),
            container: guid(&props, AEP_CONTAINER),
            connected: boolean(&props, IS_CONNECTED).unwrap_or(false),
            audio: number(&props, COD_MAJOR) == Some(COD_AUDIO_VIDEO),
        };
        state.endpoints.insert(id.to_string(), endpoint);
    } else {
        let node = Node {
            container: guid(&props, CONTAINER),
            level: battery(&props),
        };
        state.nodes.insert(id.to_string(), node);
    }
}

fn apply(state: &mut State, which: usize, update: &DeviceInformationUpdate) {
    let Ok(id) = update.Id() else { return };
    let Ok(props) = update.Properties() else {
        return;
    };
    let id = id.to_string();
    if which == 0 {
        let endpoint = state.endpoints.entry(id).or_default();
        if let Some(connected) = boolean(&props, IS_CONNECTED) {
            endpoint.connected = connected;
        }
        if let Some(name) = text(&props, "System.ItemNameDisplay") {
            endpoint.name = name;
        }
    } else {
        let node = state.nodes.entry(id).or_default();
        if props.HasKey(&HSTRING::from(BATTERY)).unwrap_or(false) {
            node.level = battery(&props);
        }
        if let Some(container) = guid(&props, CONTAINER) {
            node.container = Some(container);
        }
    }
}

type Props = IMapView<HSTRING, IInspectable>;

fn value(props: &Props, key: &str) -> Option<IPropertyValue> {
    props.Lookup(&HSTRING::from(key)).ok()?.cast().ok()
}

fn boolean(props: &Props, key: &str) -> Option<bool> {
    value(props, key)?.GetBoolean().ok()
}

fn text(props: &Props, key: &str) -> Option<String> {
    Some(value(props, key)?.GetString().ok()?.to_string())
}

fn guid(props: &Props, key: &str) -> Option<String> {
    Some(format!("{:?}", value(props, key)?.GetGuid().ok()?))
}

/// Entier, quel que soit le type exact choisi par le pilote.
fn number(props: &Props, key: &str) -> Option<u32> {
    let v = value(props, key)?;
    match v.Type().ok()? {
        PropertyType::UInt8 => v.GetUInt8().ok().map(u32::from),
        PropertyType::UInt16 => v.GetUInt16().ok().map(u32::from),
        PropertyType::UInt32 => v.GetUInt32().ok(),
        PropertyType::Int32 => v.GetInt32().ok().and_then(|n| u32::try_from(n).ok()),
        _ => None,
    }
}

fn battery(props: &Props) -> Option<u8> {
    number(props, BATTERY)
        .filter(|l| *l <= 100)
        .map(|l| l as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Énumère les appareils réels pendant quelques secondes (valide les
    /// filtres AQS) : `cargo test -p bw-power -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn enumerates_real_devices() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let wake = spawn(tx);
        std::thread::sleep(std::time::Duration::from_secs(4));
        let _ = wake.send(Wake::Stop);
        let mut got = false;
        while let Ok(devices) = rx.try_recv() {
            println!("{devices:#?}");
            got = true;
        }
        assert!(got, "aucune liste publiée : énumération incomplète ?");
    }
}
