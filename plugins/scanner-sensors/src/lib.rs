use tauri::{
    Manager, Runtime,
    plugin::{Builder, TauriPlugin},
};

mod error;
mod models;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

pub use error::{Error, Result};
pub use models::{SensorCapabilities, StartSessionOptions, TorchOptions};

#[cfg(desktop)]
pub use desktop::ScannerSensors;
#[cfg(mobile)]
pub use mobile::ScannerSensors;

pub trait ScannerSensorsExt<R: Runtime> {
    fn scanner_sensors(&self) -> &ScannerSensors<R>;
}

impl<R: Runtime, T: Manager<R>> ScannerSensorsExt<R> for T {
    fn scanner_sensors(&self) -> &ScannerSensors<R> {
        self.state::<ScannerSensors<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("scanner-sensors")
        .setup(|app, api| {
            #[cfg(mobile)]
            let sensors = mobile::init(app, api)?;
            #[cfg(desktop)]
            let sensors = desktop::init(app, api)?;
            app.manage(sensors);
            Ok(())
        })
        .build()
}
