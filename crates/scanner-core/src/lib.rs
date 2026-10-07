use serde::{Deserialize, Serialize};

pub mod clock;
pub mod guidance;
pub mod observation;
pub mod reconstruction;
pub mod repair;
pub mod session;
pub mod spatial;
pub mod structural;

pub use guidance::{
    choose_next_action, recommend_for_confidence, ConfidenceField, GuidanceCandidate,
    MeasurementAction,
};
pub use observation::{
    CameraFrameObservation, CameraIntrinsics, ClockDomain, DepthObservation, ExposureMetadata,
    IlluminationMode, IlluminationObservation, ImuObservation, ObservationEnvelope,
    ObservationLedger, ObservationRejection, ObservationSource, Pose, Quaternion,
    RangingObservation, RawObservation, Timestamp, UserCorrespondenceObservation, Vec3,
    CAPTURE_SCHEMA_VERSION,
};
pub use reconstruction::{
    FusionConfig, FusionConfigError, IntegrationReport, PreviewMesh, RaycastHit, SampleRejection,
    SparseSurfaceVolume, SurfacePoint, SurfaceSample, VoxelKey,
};
pub use repair::{
    select_repair_region, RepairNeeds, RepairRegion, RepairSelectionError,
};
pub use session::{ScanSession, SESSION_SCHEMA_VERSION};
pub use spatial::{
    ConstraintEvaluation, ConstraintRejection, ConstraintSource, SessionWorld, SolveReport,
    SpatialConstraint, SpatialConstraintKind, Uncertainty,
};
pub use structural::{
    LockState, StructuralSurface, StructuralSurfaceKind, SurfaceEvent, SurfaceTransitionError,
};

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
