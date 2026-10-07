use scanner_core::{PlatformCapabilities, SpatialCapability};
use tauri::{AppHandle, Runtime};
use tauri_plugin_scanner_sensors::{
    ScannerSensorsExt, SensorCapabilities, SensorPermissionState, StartSessionOptions, TorchOptions,
};

fn map_sensor_capabilities(capabilities: SensorCapabilities) -> Vec<SpatialCapability> {
    let mut mapped = Vec::new();

    if capabilities.camera {
        mapped.push(SpatialCapability::Camera);
    }
    if capabilities.imu {
        mapped.push(SpatialCapability::Imu);
    }
    if capabilities.depth {
        mapped.push(SpatialCapability::Depth);
    }
    if capabilities.lidar {
        mapped.push(SpatialCapability::Lidar);
    }
    if capabilities.flash_hardware {
        mapped.push(SpatialCapability::Flash);
    }
    if capabilities.hdr {
        mapped.push(SpatialCapability::Hdr);
    }

    mapped
}

#[tauri::command]
fn platform_capabilities<R: Runtime>(app: AppHandle<R>) -> PlatformCapabilities {
    #[cfg(mobile)]
    let capabilities = app
        .scanner_sensors()
        .capabilities()
        .map(map_sensor_capabilities)
        .unwrap_or_default();

    #[cfg(desktop)]
    let capabilities = Vec::<SpatialCapability>::new();

    PlatformCapabilities {
        runtime: "tauri".into(),
        mobile: cfg!(mobile),
        capabilities,
    }
}

#[tauri::command]
fn sensor_capabilities<R: Runtime>(app: AppHandle<R>) -> Result<SensorCapabilities, String> {
    app.scanner_sensors()
        .capabilities()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn sensor_permissions<R: Runtime>(app: AppHandle<R>) -> Result<SensorPermissionState, String> {
    app.scanner_sensors()
        .check_permissions()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn request_sensor_permissions<R: Runtime>(
    app: AppHandle<R>,
) -> Result<SensorPermissionState, String> {
    app.scanner_sensors()
        .request_permissions()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn start_sensor_session<R: Runtime>(
    app: AppHandle<R>,
    options: StartSessionOptions,
) -> Result<(), String> {
    app.scanner_sensors()
        .start_session(options)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn stop_sensor_session<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    app.scanner_sensors()
        .stop_session()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_sensor_torch<R: Runtime>(
    app: AppHandle<R>,
    options: TorchOptions,
) -> Result<(), String> {
    app.scanner_sensors()
        .set_torch(options)
        .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_scanner_sensors::init())
        .invoke_handler(tauri::generate_handler![
            platform_capabilities,
            sensor_capabilities,
            sensor_permissions,
            request_sensor_permissions,
            start_sensor_session,
            stop_sensor_session,
            set_sensor_torch
        ])
        .run(tauri::generate_context!())
        .expect("error while running Scanner");
}
