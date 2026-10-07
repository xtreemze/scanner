use tauri::{AppHandle, Runtime, plugin::PluginApi};

use crate::{
    Error, Result,
    models::{SensorCapabilities, StartSessionOptions, TorchOptions},
};

pub struct ScannerSensors<R: Runtime> {
    _marker: std::marker::PhantomData<R>,
}

pub fn init<R: Runtime, C: serde::de::DeserializeOwned>(
    _app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> Result<ScannerSensors<R>> {
    Ok(ScannerSensors {
        _marker: std::marker::PhantomData,
    })
}

impl<R: Runtime> ScannerSensors<R> {
    pub fn capabilities(&self) -> Result<SensorCapabilities> {
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
