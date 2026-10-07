use scanner_core::{PlatformCapabilities, SpatialCapability};

#[tauri::command]
fn platform_capabilities() -> PlatformCapabilities {
    PlatformCapabilities {
        runtime: "tauri".into(),
        mobile: cfg!(mobile),
        // Native mobile sensor adapters are not implemented yet. Do not advertise
        // platform capabilities until an adapter can actually provide them.
        capabilities: Vec::<SpatialCapability>::new(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![platform_capabilities])
        .run(tauri::generate_context!())
        .expect("error while running Scanner");
}
