use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpatialCapability {
    Camera,
    Imu,
    Depth,
    Lidar,
    Uwb,
    BluetoothRanging,
    Flash,
    Hdr,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCapabilities {
    pub runtime: String,
    pub mobile: bool,
    pub capabilities: Vec<SpatialCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LockState {
    Candidate,
    Stable,
    SuggestedLock,
    UserConfirmed,
    Locked,
    Challenged,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructuralSurface {
    pub id: String,
    pub kind: String,
    pub normal: Vec3,
    pub offset_meters: f64,
    pub confidence: f32,
    pub lock_state: LockState,
}
