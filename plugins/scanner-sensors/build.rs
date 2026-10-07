const COMMANDS: &[&str] = &[
    "capabilities",
    "start_session",
    "stop_session",
    "set_torch",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .ios_path("ios")
        .build();
}
