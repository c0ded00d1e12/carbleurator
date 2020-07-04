use anyhow::Result;
use thiserror::Error;
// Bring in required traits
use anyhow::Context;
use btleplug::api::{Central, Peripheral};
use failure::Fail;

mod signaling;
use crate::signaling::{update_signal_failure, update_signal_progress, update_signal_success};

#[cfg(target_os = "linux")]
use btleplug::bluez::{adapter::ConnectedAdapter as BleAdapter, manager::Manager as BleManager};
#[cfg(target_os = "macos")]
use btleplug::corebluetooth::{adapter::Adapter as BleAdapter, manager::Manager as BleManager};
#[cfg(target_os = "windows")]
use btleplug::winrtble::{adapter::Adapter as BleAdapter, manager::Manager as BleManager};

#[derive(Error, Debug)]
pub enum CarbleuratorError {
    #[error("USB not supported")]
    UsbNotSupportedError,
    #[error("USB device initialization error")]
    UsbDeviceInitializationError,
    #[error("USB initialization error")]
    UsbInitializationError(Box<dyn std::error::Error + Send + Sync>),
    #[error("No USB gamepads found")]
    MissingGamepad,
    #[error("No BLE adapters found")]
    MissingBleAdapter,
}

impl From<gilrs::Error> for CarbleuratorError {
    fn from(err: gilrs::Error) -> Self {
        match err {
            gilrs::Error::NotImplemented(_) => Self::UsbNotSupportedError,
            gilrs::Error::InvalidAxisToBtn => Self::UsbDeviceInitializationError,
            gilrs::Error::Other(e) => Self::UsbInitializationError(e),
        }
    }
}

pub(crate) fn init_gamepads() -> Result<gilrs::Gilrs> {
    let gilrs = gilrs::Gilrs::new().map_err(CarbleuratorError::from)?;
    if gilrs.gamepads().count() == 0 {
        return Err(CarbleuratorError::MissingGamepad.into());
    }
    Ok(gilrs)
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn get_central(manager: &BleManager) -> Result<BleAdapter> {
    manager
        .adapters()?
        .compat()?
        .first()
        .ok_or(CarbleuratorError::MissingBleAdapter)
}

#[cfg(target_os = "linux")]
fn get_central(manager: &BleManager) -> Result<BleAdapter> {
    let adapters = manager.adapters().map_err(|e| e.compat())?;
    let adapter = adapters
        .first()
        .ok_or(CarbleuratorError::MissingBleAdapter)?;
    adapter.connect().map_err(|e| e.compat().into())
}

fn main() -> Result<()> {
    update_signal_progress();
    // Init gamepads
    let mut gilrs = init_gamepads()?;
    for (_id, gamepad) in gilrs.gamepads() {
        println!("{} is {:?}", gamepad.name(), gamepad.power_info());
    }
    update_signal_progress();

    // Init bluetooth
    let manager = BleManager::new()
        .map_err(|e| e.compat())?;

    update_signal_progress();

    let central = get_central(&manager)?;

    central
        .start_scan()
        .map_err(|e| e.compat())
        .with_context(|| "Failed to scan for new BLE peripherals".to_string())?;

    update_signal_progress();
    std::thread::sleep(std::time::Duration::from_secs(2));
    update_signal_progress();

    for peripheral in central.peripherals() {
        println!(
            "{} ({})",
            peripheral.properties().local_name.unwrap_or_default(),
            peripheral.properties().address
        );
    }

    update_signal_success();
    // Start event loop
    loop {
        while let Some(gilrs::Event { id, event, time }) = gilrs.next_event() {
            println!("{:?} New event from {}: {:?}", time, id, event);
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
