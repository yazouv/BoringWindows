//! Batterie du PC via `Windows.System.Power.PowerManager` : trois événements
//! (état de la batterie, alimentation, pourcentage), relus à chaque signal.

use std::sync::mpsc::{Receiver, Sender, channel};

use tokio::sync::mpsc::UnboundedSender;
use windows::Foundation::EventHandler;
use windows::System::Power::{BatteryStatus, PowerManager, PowerSupplyStatus};
use windows_core::IInspectable;

use crate::{Battery, Wake};

fn read() -> windows::core::Result<Battery> {
    let status = PowerManager::BatteryStatus()?;
    Ok(Battery {
        present: status != BatteryStatus::NotPresent,
        plugged: PowerManager::PowerSupplyStatus()? != PowerSupplyStatus::NotPresent,
        percent: PowerManager::RemainingChargePercent()?.clamp(0, 100) as u8,
    })
}

/// Thread dédié : inscription aux événements, puis relecture à chaque signal
/// jusqu'à `Wake::Stop` (envoyé à l'arrêt du module).
pub fn spawn(tx: UnboundedSender<Battery>) -> Sender<Wake> {
    let (wake_tx, wake_rx) = channel();
    let handler_tx = wake_tx.clone();
    std::thread::spawn(move || {
        if let Err(e) = run(&tx, handler_tx, &wake_rx) {
            log::warn!("batterie : état indisponible : {e}");
        }
    });
    wake_tx
}

fn run(
    tx: &UnboundedSender<Battery>,
    wake_tx: Sender<Wake>,
    wake: &Receiver<Wake>,
) -> windows::core::Result<()> {
    let first = read()?;
    if !first.present {
        log::info!("batterie : aucune (PC de bureau)");
    }
    let _ = tx.send(first);

    let handler = EventHandler::<IInspectable>::new(move |_, _| {
        let _ = wake_tx.send(Wake::Changed);
        Ok(())
    });
    let tokens = [
        PowerManager::BatteryStatusChanged(&handler)?,
        PowerManager::PowerSupplyStatusChanged(&handler)?,
        PowerManager::RemainingChargePercentChanged(&handler)?,
    ];

    let mut last = first;
    while let Ok(first) = wake.recv() {
        // Une rafale d'événements ne donne qu'une relecture.
        if std::iter::once(first)
            .chain(wake.try_iter())
            .any(|w| w == Wake::Stop)
        {
            break;
        }
        match read() {
            Ok(now) if now != last => {
                last = now;
                if tx.send(now).is_err() {
                    break;
                }
            }
            Ok(_) => {}
            Err(e) => log::debug!("batterie : lecture : {e}"),
        }
    }

    let _ = PowerManager::RemoveBatteryStatusChanged(tokens[0]);
    let _ = PowerManager::RemovePowerSupplyStatusChanged(tokens[1]);
    let _ = PowerManager::RemoveRemainingChargePercentChanged(tokens[2]);
    Ok(())
}
