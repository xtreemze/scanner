use serde::{Deserialize, Serialize};

pub const CAPTURE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClockDomain {
    SessionMonotonic,
    DeviceMonotonic,
    CameraSensor,
    Platform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ObservationSource {
    Camera,
    Imu,
    Depth,
    Lidar,
    Uwb,
    BluetoothRanging,
    VisualPeer,
    User,
    Illumination,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Timestamp {
    pub micros: u64,
    pub clock_domain: ClockDomain,
    pub uncertainty_micros: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quaternion {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pose {
    pub position_meters: Vec3,
    pub orientation: Quaternion,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraIntrinsics {
    pub width_px: u32,
    pub height_px: u32,
    pub fx: f64,
    pub fy: f64,
    pub cx: f64,
    pub cy: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExposureMetadata {
    pub exposure_seconds: f64,
    pub iso: f64,
    pub aperture_f_number: Option<f64>,
    pub white_balance_kelvin: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservationEnvelope<T> {
    pub id: String,
    pub device_id: String,
    pub epoch: u64,
    pub captured_at: Timestamp,
    pub source: ObservationSource,
    pub payload: T,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraFrameObservation {
    pub intrinsics: CameraIntrinsics,
    pub pose_device_local: Pose,
    pub exposure: ExposureMetadata,
    pub frame_asset_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImuObservation {
    pub acceleration_mps2: Vec3,
    pub angular_velocity_rps: Vec3,
    pub gravity_mps2: Option<Vec3>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepthObservation {
    pub width_px: u32,
    pub height_px: u32,
    pub depth_asset_id: String,
    pub confidence_asset_id: Option<String>,
    pub min_depth_meters: f32,
    pub max_depth_meters: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RangingObservation {
    pub peer_device_id: String,
    pub distance_meters: f64,
    pub standard_deviation_meters: f64,
    pub direction_device_local: Option<Vec3>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IlluminationObservation {
    pub emitter_device_id: String,
    pub mode: IlluminationMode,
    pub intensity_normalized: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IlluminationMode {
    Ambient,
    Torch,
    FlashPulse,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserCorrespondenceObservation {
    pub landmark_id: String,
    pub image_x_normalized: f32,
    pub image_y_normalized: f32,
    pub semantic_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "kebab-case")]
pub enum RawObservation {
    CameraFrame(ObservationEnvelope<CameraFrameObservation>),
    Imu(ObservationEnvelope<ImuObservation>),
    Depth(ObservationEnvelope<DepthObservation>),
    Ranging(ObservationEnvelope<RangingObservation>),
    Illumination(ObservationEnvelope<IlluminationObservation>),
    UserCorrespondence(ObservationEnvelope<UserCorrespondenceObservation>),
}

impl RawObservation {
    pub fn id(&self) -> &str {
        match self {
            Self::CameraFrame(v) => &v.id,
            Self::Imu(v) => &v.id,
            Self::Depth(v) => &v.id,
            Self::Ranging(v) => &v.id,
            Self::Illumination(v) => &v.id,
            Self::UserCorrespondence(v) => &v.id,
        }
    }

    pub fn epoch(&self) -> u64 {
        match self {
            Self::CameraFrame(v) => v.epoch,
            Self::Imu(v) => v.epoch,
            Self::Depth(v) => v.epoch,
            Self::Ranging(v) => v.epoch,
            Self::Illumination(v) => v.epoch,
            Self::UserCorrespondence(v) => v.epoch,
        }
    }

    pub fn captured_at(&self) -> Timestamp {
        match self {
            Self::CameraFrame(v) => v.captured_at,
            Self::Imu(v) => v.captured_at,
            Self::Depth(v) => v.captured_at,
            Self::Ranging(v) => v.captured_at,
            Self::Illumination(v) => v.captured_at,
            Self::UserCorrespondence(v) => v.captured_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservationRejection {
    StaleEpoch { expected: u64, actual: u64 },
    DuplicateId,
}

#[derive(Debug, Default)]
pub struct ObservationLedger {
    epoch: u64,
    last_timestamp_micros: Option<u64>,
    accepted_ids: std::collections::BTreeSet<String>,
}

impl ObservationLedger {
    pub fn new(epoch: u64) -> Self {
        Self {
            epoch,
            ..Self::default()
        }
    }

    pub fn accept(&mut self, observation: &RawObservation) -> Result<(), ObservationRejection> {
        if observation.epoch() != self.epoch {
            return Err(ObservationRejection::StaleEpoch {
                expected: self.epoch,
                actual: observation.epoch(),
            });
        }
        if !self.accepted_ids.insert(observation.id().to_owned()) {
            return Err(ObservationRejection::DuplicateId);
        }
        let timestamp = observation.captured_at().micros;
        self.last_timestamp_micros = Some(
            self.last_timestamp_micros
                .map_or(timestamp, |last| last.max(timestamp)),
        );
        Ok(())
    }

    pub fn last_timestamp_micros(&self) -> Option<u64> {
        self.last_timestamp_micros
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn imu(id: &str, epoch: u64, micros: u64) -> RawObservation {
        RawObservation::Imu(ObservationEnvelope {
            id: id.into(),
            device_id: "phone-a".into(),
            epoch,
            captured_at: Timestamp {
                micros,
                clock_domain: ClockDomain::SessionMonotonic,
                uncertainty_micros: 50,
            },
            source: ObservationSource::Imu,
            payload: ImuObservation {
                acceleration_mps2: Vec3 { x: 0.0, y: 0.0, z: 9.81 },
                angular_velocity_rps: Vec3 { x: 0.0, y: 0.0, z: 0.0 },
                gravity_mps2: None,
            },
        })
    }

    #[test]
    fn rejects_stale_epoch_without_rewriting_session_generation() {
        let mut ledger = ObservationLedger::new(4);
        let result = ledger.accept(&imu("old", 3, 10));
        assert_eq!(
            result,
            Err(ObservationRejection::StaleEpoch {
                expected: 4,
                actual: 3
            })
        );
    }

    #[test]
    fn rejects_duplicate_observation_identity() {
        let mut ledger = ObservationLedger::new(2);
        assert_eq!(ledger.accept(&imu("same", 2, 100)), Ok(()));
        assert_eq!(
            ledger.accept(&imu("same", 2, 101)),
            Err(ObservationRejection::DuplicateId)
        );
    }

    #[test]
    fn permits_out_of_order_arrival_but_preserves_monotonic_watermark() {
        let mut ledger = ObservationLedger::new(1);
        ledger.accept(&imu("later", 1, 200)).unwrap();
        ledger.accept(&imu("earlier", 1, 100)).unwrap();
        assert_eq!(ledger.last_timestamp_micros(), Some(200));
    }
}
