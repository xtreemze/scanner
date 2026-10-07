use tauri::{AppHandle, Runtime, plugin::PluginApi};

use crate::{
    Error, Result,
    models::{SensorCapabilities, SensorPermissionState, StartSessionOptions, TorchOptions},
};

pub struct ScannerSensors<R: Runtime>(AppHandle<R>);

pub fn init<R: Runtime, C: serde::de::DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> Result<ScannerSensors<R>> {
    Ok(ScannerSensors(app.clone()))
}

impl<R: Runtime> ScannerSensors<R> {
    pub fn capabilities(&self) -> Result<SensorCapabilities> {
        let _ = &self.0;
        Err(Error::UnsupportedPlatform)
    }

    pub fn check_permissions(&self) -> Result<SensorPermissionState> {
        Err(Error::UnsupportedPlatform)
    }

    pub fn request_permissions(&self) -> Result<SensorPermissionState> {
        Err(Error::UnsupportedPlatform)
    }

    pub fn start_session(&self, _options: StartSessionOptions) -> Result<()> {
        Err(Error::UnsupportedPlatform)
    }

    pub fn stop_session(&self) -> Result<()> {
        Err(Error::UnsupportedPlatform)
    }

    pub fn set_torch(&self, _options: TorchOptions) -> Result<()> {
        Err(Error::UnsupportedPlatform)
    }
}
