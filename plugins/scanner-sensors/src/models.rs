use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensorCapabilities {
    pub camera: bool,
    pub imu: bool,
    pub depth: bool,
    pub lidar: bool,
    pub scene_mesh: bool,
    pub flash_hardware: bool,
    pub flash_session_control: bool,
    pub hdr: bool,
    pub platform_tracking: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionState {
    Prompt,
    Granted,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensorPermissionState {
    pub camera: PermissionState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartSessionOptions {
    pub device_id: String,
    pub epoch: u64,
    pub reset_tracking: bool,
    pub prefer_raw_depth: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorchOptions {
    pub level: Option<f32>,
}
