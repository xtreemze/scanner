use serde::de::DeserializeOwned;
use tauri::{
    AppHandle, Runtime,
    plugin::{PluginApi, PluginHandle},
};

use crate::{
    Result,
    models::{SensorCapabilities, StartSessionOptions, TorchOptions},
};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "com.xtreemze.scanner.sensors";

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_scanner_sensors);

pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> Result<ScannerSensors<R>> {
    #[cfg(target_os = "android")]
    let handle =
        api.register_android_plugin(PLUGIN_IDENTIFIER, "ScannerSensorsPlugin")?;

    #[cfg(target_os = "ios")]
    let handle = api.register_ios_plugin(init_plugin_scanner_sensors)?;

    Ok(ScannerSensors(handle))
}

pub struct ScannerSensors<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> ScannerSensors<R> {
    pub fn capabilities(&self) -> Result<SensorCapabilities> {
        self.0
            .run_mobile_plugin("capabilities", ())
            .map_err(Into::into)
    }

    pub fn start_session(&self, options: StartSessionOptions) -> Result<()> {
        self.0
            .run_mobile_plugin("startSession", options)
            .map_err(Into::into)
    }

    pub fn stop_session(&self) -> Result<()> {
        self.0
            .run_mobile_plugin("stopSession", ())
            .map_err(Into::into)
    }

    pub fn set_torch(&self, options: TorchOptions) -> Result<()> {
        self.0
            .run_mobile_plugin("setTorch", options)
            .map_err(Into::into)
    }
}
