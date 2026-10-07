use std::{
    collections::VecDeque,
    slice,
    str,
};

use crate::observation::{
    CameraFrameObservation, CameraIntrinsics, ClockDomain, DepthObservation, ExposureMetadata,
    ImuObservation, ObservationEnvelope, ObservationLedger, ObservationSource, Pose, Quaternion,
    RawObservation, Timestamp, Vec3,
};

const MAX_OBSERVATIONS: usize = 256;
const MAX_DEPTH_FRAMES: usize = 4;

const OPTIONAL_APERTURE: u32 = 1 << 0;
const OPTIONAL_WHITE_BALANCE: u32 = 1 << 1;

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeStatus {
    Ok = 0,
    NullHandle = 1,
    NullPointer = 2,
    InvalidUtf8 = 3,
    InvalidClockDomain = 4,
    InvalidMetadata = 5,
    InvalidBuffer = 6,
    DuplicateObservation = 7,
    StaleEpoch = 8,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeDepthFormat {
    U16Millimeters = 1,
    F32Meters = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NativeVec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NativeQuaternion {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NativePose {
    pub position_meters: NativeVec3,
    pub orientation: NativeQuaternion,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NativeCameraIntrinsics {
    pub width_px: u32,
    pub height_px: u32,
    pub fx: f64,
    pub fy: f64,
    pub cx: f64,
    pub cy: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NativeCameraFrameMetadata {
    pub timestamp_micros: u64,
    pub clock_domain: u32,
    pub uncertainty_micros: u32,
    pub pose_device_local: NativePose,
    pub intrinsics: NativeCameraIntrinsics,
    pub exposure_seconds: f64,
    pub iso: f64,
    pub aperture_f_number: f64,
    pub white_balance_kelvin: f64,
    pub optional_fields: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NativeImuSample {
    pub timestamp_micros: u64,
    pub clock_domain: u32,
    pub uncertainty_micros: u32,
    pub acceleration_mps2: NativeVec3,
    pub angular_velocity_rps: NativeVec3,
    pub gravity_mps2: NativeVec3,
    pub has_gravity: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NativeDepthDescriptor {
    pub timestamp_micros: u64,
    pub clock_domain: u32,
    pub uncertainty_micros: u32,
    pub width_px: u32,
    pub height_px: u32,
    pub depth_format: u32,
    pub depth_row_stride_bytes: usize,
    pub confidence_row_stride_bytes: usize,
    pub min_depth_meters: f32,
    pub max_depth_meters: f32,
    pub fresh_for_frame: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeIngestStats {
    pub accepted_camera_frames: u64,
    pub accepted_imu_samples: u64,
    pub accepted_depth_frames: u64,
    pub dropped_observations: u64,
    pub dropped_depth_frames: u64,
    pub queued_observations: usize,
    pub queued_depth_frames: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeDepthFrame {
    pub observation_id: String,
    pub device_id: String,
    pub epoch: u64,
    pub captured_at: Timestamp,
    pub width_px: u32,
    pub height_px: u32,
    pub format: NativeDepthFormat,
    pub depth_row_stride_bytes: usize,
    pub confidence_row_stride_bytes: Option<usize>,
    pub fresh_for_frame: bool,
    pub depth_bytes: Vec<u8>,
    pub confidence_bytes: Option<Vec<u8>>,
}

#[derive(Debug)]
pub struct NativeIngestSession {
    device_id: String,
    epoch: u64,
    sequence: u64,
    ledger: ObservationLedger,
    observations: VecDeque<RawObservation>,
    depth_frames: VecDeque<NativeDepthFrame>,
    stats: NativeIngestStats,
}

impl NativeIngestSession {
    pub fn new(device_id: impl Into<String>, epoch: u64) -> Option<Self> {
        let device_id = device_id.into();
        if device_id.trim().is_empty() {
            return None;
        }

        Some(Self {
            device_id,
            epoch,
            sequence: 0,
            ledger: ObservationLedger::new(epoch),
            observations: VecDeque::new(),
            depth_frames: VecDeque::new(),
            stats: NativeIngestStats::default(),
        })
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn stats(&self) -> NativeIngestStats {
        let mut stats = self.stats;
        stats.queued_observations = self.observations.len();
        stats.queued_depth_frames = self.depth_frames.len();
        stats
    }

    pub fn pop_observation(&mut self) -> Option<RawObservation> {
        self.observations.pop_front()
    }

    pub fn pop_depth_frame(&mut self) -> Option<NativeDepthFrame> {
        self.depth_frames.pop_front()
    }

    pub fn ingest_camera_metadata(
        &mut self,
        metadata: NativeCameraFrameMetadata,
    ) -> NativeStatus {
        let Some(timestamp) = timestamp(
            metadata.timestamp_micros,
            metadata.clock_domain,
            metadata.uncertainty_micros,
        ) else {
            return NativeStatus::InvalidClockDomain;
        };
        let Some(pose) = pose(metadata.pose_device_local) else {
            return NativeStatus::InvalidMetadata;
        };
        let Some(intrinsics) = intrinsics(metadata.intrinsics) else {
            return NativeStatus::InvalidMetadata;
        };
        if !metadata.exposure_seconds.is_finite()
            || metadata.exposure_seconds < 0.0
            || !metadata.iso.is_finite()
            || metadata.iso < 0.0
        {
            return NativeStatus::InvalidMetadata;
        }

        let aperture = if metadata.optional_fields & OPTIONAL_APERTURE != 0 {
            if !metadata.aperture_f_number.is_finite() || metadata.aperture_f_number <= 0.0 {
                return NativeStatus::InvalidMetadata;
            }
            Some(metadata.aperture_f_number)
        } else {
            None
        };

        let white_balance = if metadata.optional_fields & OPTIONAL_WHITE_BALANCE != 0 {
            if !metadata.white_balance_kelvin.is_finite()
                || metadata.white_balance_kelvin <= 0.0
            {
                return NativeStatus::InvalidMetadata;
            }
            Some(metadata.white_balance_kelvin)
        } else {
            None
        };

        let id = self.next_id("camera");
        let observation = RawObservation::CameraFrame(ObservationEnvelope {
            id: id.clone(),
            device_id: self.device_id.clone(),
            epoch: self.epoch,
            captured_at: timestamp,
            source: ObservationSource::Camera,
            payload: CameraFrameObservation {
                intrinsics,
                pose_device_local: pose,
                exposure: ExposureMetadata {
                    exposure_seconds: metadata.exposure_seconds,
                    iso: metadata.iso,
                    aperture_f_number: aperture,
                    white_balance_kelvin: white_balance,
                },
                frame_asset_id: format!("native-frame:{id}"),
            },
        });

        match self.enqueue_observation(observation) {
            NativeStatus::Ok => {
                self.stats.accepted_camera_frames =
                    self.stats.accepted_camera_frames.saturating_add(1);
                NativeStatus::Ok
            }
            status => status,
        }
    }

    pub fn ingest_imu(&mut self, sample: NativeImuSample) -> NativeStatus {
        let Some(timestamp) = timestamp(
            sample.timestamp_micros,
            sample.clock_domain,
            sample.uncertainty_micros,
        ) else {
            return NativeStatus::InvalidClockDomain;
        };
        let Some(acceleration) = vec3(sample.acceleration_mps2) else {
            return NativeStatus::InvalidMetadata;
        };
        let Some(angular_velocity) = vec3(sample.angular_velocity_rps) else {
            return NativeStatus::InvalidMetadata;
        };
        let gravity = if sample.has_gravity != 0 {
            let Some(gravity) = vec3(sample.gravity_mps2) else {
                return NativeStatus::InvalidMetadata;
            };
            Some(gravity)
        } else {
            None
        };

        let observation = RawObservation::Imu(ObservationEnvelope {
            id: self.next_id("imu"),
            device_id: self.device_id.clone(),
            epoch: self.epoch,
            captured_at: timestamp,
            source: ObservationSource::Imu,
            payload: ImuObservation {
                acceleration_mps2: acceleration,
                angular_velocity_rps: angular_velocity,
                gravity_mps2: gravity,
            },
        });

        match self.enqueue_observation(observation) {
            NativeStatus::Ok => {
                self.stats.accepted_imu_samples =
                    self.stats.accepted_imu_samples.saturating_add(1);
                NativeStatus::Ok
            }
            status => status,
        }
    }

    pub fn ingest_depth(
        &mut self,
        descriptor: NativeDepthDescriptor,
        depth_bytes: &[u8],
        confidence_bytes: Option<&[u8]>,
    ) -> NativeStatus {
        let Some(timestamp) = timestamp(
            descriptor.timestamp_micros,
            descriptor.clock_domain,
            descriptor.uncertainty_micros,
        ) else {
            return NativeStatus::InvalidClockDomain;
        };

        let unspecified_range =
            descriptor.min_depth_meters == 0.0 && descriptor.max_depth_meters == 0.0;
        if descriptor.width_px == 0
            || descriptor.height_px == 0
            || !descriptor.min_depth_meters.is_finite()
            || !descriptor.max_depth_meters.is_finite()
            || descriptor.min_depth_meters < 0.0
            || (!unspecified_range
                && descriptor.max_depth_meters < descriptor.min_depth_meters)
        {
            return NativeStatus::InvalidMetadata;
        }

        let format = match descriptor.depth_format {
            x if x == NativeDepthFormat::U16Millimeters as u32 => {
                NativeDepthFormat::U16Millimeters
            }
            x if x == NativeDepthFormat::F32Meters as u32 => NativeDepthFormat::F32Meters,
            _ => return NativeStatus::InvalidMetadata,
        };

        let bytes_per_depth_pixel = match format {
            NativeDepthFormat::U16Millimeters => 2usize,
            NativeDepthFormat::F32Meters => 4usize,
        };
        let minimum_depth_row = descriptor.width_px as usize * bytes_per_depth_pixel;
        if descriptor.depth_row_stride_bytes < minimum_depth_row {
            return NativeStatus::InvalidBuffer;
        }
        let required_depth_len = match descriptor
            .depth_row_stride_bytes
            .checked_mul(descriptor.height_px as usize)
        {
            Some(len) => len,
            None => return NativeStatus::InvalidBuffer,
        };
        if depth_bytes.len() < required_depth_len {
            return NativeStatus::InvalidBuffer;
        }

        let confidence = match confidence_bytes {
            Some(bytes) => {
                if descriptor.confidence_row_stride_bytes < descriptor.width_px as usize {
                    return NativeStatus::InvalidBuffer;
                }
                let required_len = match descriptor
                    .confidence_row_stride_bytes
                    .checked_mul(descriptor.height_px as usize)
                {
                    Some(len) => len,
                    None => return NativeStatus::InvalidBuffer,
                };
                if bytes.len() < required_len {
                    return NativeStatus::InvalidBuffer;
                }
                Some(bytes[..required_len].to_vec())
            }
            None => None,
        };

        let (min_depth_meters, max_depth_meters) = if unspecified_range {
            let Some(range) = derive_depth_range(
                format,
                depth_bytes,
                descriptor.width_px,
                descriptor.height_px,
                descriptor.depth_row_stride_bytes,
            ) else {
                return NativeStatus::InvalidBuffer;
            };
            range
        } else {
            (descriptor.min_depth_meters, descriptor.max_depth_meters)
        };

        let id = self.next_id("depth");
        let depth_asset_id = format!("native-depth:{id}");
        let confidence_asset_id = confidence
            .as_ref()
            .map(|_| format!("native-depth-confidence:{id}"));

        let observation = RawObservation::Depth(ObservationEnvelope {
            id: id.clone(),
            device_id: self.device_id.clone(),
            epoch: self.epoch,
            captured_at: timestamp,
            source: ObservationSource::Depth,
            payload: DepthObservation {
                width_px: descriptor.width_px,
                height_px: descriptor.height_px,
                depth_asset_id,
                confidence_asset_id,
                min_depth_meters,
                max_depth_meters,
            },
        });

        let status = self.enqueue_observation(observation);
        if status != NativeStatus::Ok {
            return status;
        }

        if self.depth_frames.len() == MAX_DEPTH_FRAMES {
            self.depth_frames.pop_front();
            self.stats.dropped_depth_frames =
                self.stats.dropped_depth_frames.saturating_add(1);
        }
        self.depth_frames.push_back(NativeDepthFrame {
            observation_id: id,
            device_id: self.device_id.clone(),
            epoch: self.epoch,
            captured_at: timestamp,
            width_px: descriptor.width_px,
            height_px: descriptor.height_px,
            format,
            depth_row_stride_bytes: descriptor.depth_row_stride_bytes,
            confidence_row_stride_bytes: confidence
                .as_ref()
                .map(|_| descriptor.confidence_row_stride_bytes),
            fresh_for_frame: descriptor.fresh_for_frame != 0,
            depth_bytes: depth_bytes[..required_depth_len].to_vec(),
            confidence_bytes: confidence,
        });
        self.stats.accepted_depth_frames =
            self.stats.accepted_depth_frames.saturating_add(1);
        NativeStatus::Ok
    }

    fn next_id(&mut self, kind: &str) -> String {
        self.sequence = self.sequence.saturating_add(1);
        format!("native:{}:{kind}:{}", self.device_id, self.sequence)
    }

    fn enqueue_observation(&mut self, observation: RawObservation) -> NativeStatus {
        if let Err(error) = self.ledger.accept(&observation) {
            return match error {
                crate::observation::ObservationRejection::DuplicateId => {
                    NativeStatus::DuplicateObservation
                }
                crate::observation::ObservationRejection::StaleEpoch { .. } => {
                    NativeStatus::StaleEpoch
                }
            };
        }

        if self.observations.len() == MAX_OBSERVATIONS {
            self.observations.pop_front();
            self.stats.dropped_observations =
                self.stats.dropped_observations.saturating_add(1);
        }
        self.observations.push_back(observation);
        NativeStatus::Ok
    }
}

fn clock_domain(raw: u32) -> Option<ClockDomain> {
    match raw {
        0 => Some(ClockDomain::SessionMonotonic),
        1 => Some(ClockDomain::DeviceMonotonic),
        2 => Some(ClockDomain::CameraSensor),
        3 => Some(ClockDomain::Platform),
        _ => None,
    }
}

fn timestamp(micros: u64, domain: u32, uncertainty_micros: u32) -> Option<Timestamp> {
    Some(Timestamp {
        micros,
        clock_domain: clock_domain(domain)?,
        uncertainty_micros,
    })
}

fn vec3(value: NativeVec3) -> Option<Vec3> {
    if !value.x.is_finite() || !value.y.is_finite() || !value.z.is_finite() {
        return None;
    }
    Some(Vec3 {
        x: value.x,
        y: value.y,
        z: value.z,
    })
}

fn pose(value: NativePose) -> Option<Pose> {
    let position = vec3(value.position_meters)?;
    let q = value.orientation;
    if !q.x.is_finite() || !q.y.is_finite() || !q.z.is_finite() || !q.w.is_finite() {
        return None;
    }
    let norm = (q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w).sqrt();
    if !norm.is_finite() || norm <= 1e-9 {
        return None;
    }
    Some(Pose {
        position_meters: position,
        orientation: Quaternion {
            x: q.x / norm,
            y: q.y / norm,
            z: q.z / norm,
            w: q.w / norm,
        },
    })
}

fn intrinsics(value: NativeCameraIntrinsics) -> Option<CameraIntrinsics> {
    if value.width_px == 0
        || value.height_px == 0
        || !value.fx.is_finite()
        || !value.fy.is_finite()
        || !value.cx.is_finite()
        || !value.cy.is_finite()
        || value.fx <= 0.0
        || value.fy <= 0.0
    {
        return None;
    }
    Some(CameraIntrinsics {
        width_px: value.width_px,
        height_px: value.height_px,
        fx: value.fx,
        fy: value.fy,
        cx: value.cx,
        cy: value.cy,
    })
}

fn derive_depth_range(
    format: NativeDepthFormat,
    bytes: &[u8],
    width_px: u32,
    height_px: u32,
    row_stride_bytes: usize,
) -> Option<(f32, f32)> {
    let mut min_depth = f32::INFINITY;
    let mut max_depth = 0.0f32;

    for row in 0..height_px as usize {
        let row_start = row.checked_mul(row_stride_bytes)?;
        match format {
            NativeDepthFormat::U16Millimeters => {
                for column in 0..width_px as usize {
                    let offset = row_start.checked_add(column.checked_mul(2)?)?;
                    let pair = bytes.get(offset..offset + 2)?;
                    let millimeters = u16::from_ne_bytes([pair[0], pair[1]]);
                    if millimeters == 0 {
                        continue;
                    }
                    let meters = f32::from(millimeters) / 1000.0;
                    min_depth = min_depth.min(meters);
                    max_depth = max_depth.max(meters);
                }
            }
            NativeDepthFormat::F32Meters => {
                for column in 0..width_px as usize {
                    let offset = row_start.checked_add(column.checked_mul(4)?)?;
                    let value = bytes.get(offset..offset + 4)?;
                    let meters =
                        f32::from_ne_bytes([value[0], value[1], value[2], value[3]]);
                    if !meters.is_finite() || meters <= 0.0 {
                        continue;
                    }
                    min_depth = min_depth.min(meters);
                    max_depth = max_depth.max(meters);
                }
            }
        }
    }

    if min_depth.is_finite() && max_depth >= min_depth {
        Some((min_depth, max_depth))
    } else {
        None
    }
}

unsafe fn borrowed_bytes<'a>(ptr: *const u8, len: usize) -> Option<&'a [u8]> {
    if len == 0 {
        return Some(&[]);
    }
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { slice::from_raw_parts(ptr, len) })
}

unsafe fn session_mut<'a>(handle: *mut NativeIngestSession) -> Option<&'a mut NativeIngestSession> {
    if handle.is_null() {
        return None;
    }
    Some(unsafe { &mut *handle })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn scanner_native_session_create(
    device_id_ptr: *const u8,
    device_id_len: usize,
    epoch: u64,
) -> *mut NativeIngestSession {
    let Some(bytes) = (unsafe { borrowed_bytes(device_id_ptr, device_id_len) }) else {
        return std::ptr::null_mut();
    };
    let Ok(device_id) = str::from_utf8(bytes) else {
        return std::ptr::null_mut();
    };
    let Some(session) = NativeIngestSession::new(device_id.to_owned(), epoch) else {
        return std::ptr::null_mut();
    };
    Box::into_raw(Box::new(session))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn scanner_native_session_destroy(handle: *mut NativeIngestSession) {
    if !handle.is_null() {
        drop(unsafe { Box::from_raw(handle) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn scanner_native_ingest_camera_metadata(
    handle: *mut NativeIngestSession,
    metadata: *const NativeCameraFrameMetadata,
) -> NativeStatus {
    let Some(session) = (unsafe { session_mut(handle) }) else {
        return NativeStatus::NullHandle;
    };
    if metadata.is_null() {
        return NativeStatus::NullPointer;
    }
    session.ingest_camera_metadata(unsafe { *metadata })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn scanner_native_ingest_imu(
    handle: *mut NativeIngestSession,
    sample: *const NativeImuSample,
) -> NativeStatus {
    let Some(session) = (unsafe { session_mut(handle) }) else {
        return NativeStatus::NullHandle;
    };
    if sample.is_null() {
        return NativeStatus::NullPointer;
    }
    session.ingest_imu(unsafe { *sample })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn scanner_native_ingest_depth(
    handle: *mut NativeIngestSession,
    descriptor: *const NativeDepthDescriptor,
    depth_ptr: *const u8,
    depth_len: usize,
    confidence_ptr: *const u8,
    confidence_len: usize,
) -> NativeStatus {
    let Some(session) = (unsafe { session_mut(handle) }) else {
        return NativeStatus::NullHandle;
    };
    if descriptor.is_null() {
        return NativeStatus::NullPointer;
    }
    let Some(depth) = (unsafe { borrowed_bytes(depth_ptr, depth_len) }) else {
        return NativeStatus::NullPointer;
    };
    let confidence = if confidence_len == 0 {
        None
    } else {
        let Some(bytes) = (unsafe { borrowed_bytes(confidence_ptr, confidence_len) }) else {
            return NativeStatus::NullPointer;
        };
        Some(bytes)
    };

    session.ingest_depth(unsafe { *descriptor }, depth, confidence)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn scanner_native_ingest_stats(
    handle: *mut NativeIngestSession,
    out_stats: *mut NativeIngestStats,
) -> NativeStatus {
    let Some(session) = (unsafe { session_mut(handle) }) else {
        return NativeStatus::NullHandle;
    };
    if out_stats.is_null() {
        return NativeStatus::NullPointer;
    }
    unsafe {
        *out_stats = session.stats();
    }
    NativeStatus::Ok
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity_pose() -> NativePose {
        NativePose {
            position_meters: NativeVec3 {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            orientation: NativeQuaternion {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        }
    }

    fn camera_metadata() -> NativeCameraFrameMetadata {
        NativeCameraFrameMetadata {
            timestamp_micros: 10_000,
            clock_domain: 2,
            uncertainty_micros: 100,
            pose_device_local: identity_pose(),
            intrinsics: NativeCameraIntrinsics {
                width_px: 1920,
                height_px: 1080,
                fx: 1000.0,
                fy: 1000.0,
                cx: 960.0,
                cy: 540.0,
            },
            exposure_seconds: 1.0 / 120.0,
            iso: 80.0,
            aperture_f_number: 1.8,
            white_balance_kelvin: 5000.0,
            optional_fields: OPTIONAL_APERTURE | OPTIONAL_WHITE_BALANCE,
        }
    }

    #[test]
    fn camera_metadata_becomes_canonical_observation() {
        let mut session = NativeIngestSession::new("phone-a", 7).unwrap();
        assert_eq!(
            session.ingest_camera_metadata(camera_metadata()),
            NativeStatus::Ok
        );

        let observation = session.pop_observation().unwrap();
        let RawObservation::CameraFrame(frame) = observation else {
            panic!("expected camera observation");
        };
        assert_eq!(frame.device_id, "phone-a");
        assert_eq!(frame.epoch, 7);
        assert_eq!(frame.captured_at.clock_domain, ClockDomain::CameraSensor);
        assert_eq!(frame.payload.intrinsics.width_px, 1920);
        assert_eq!(frame.payload.exposure.iso, 80.0);
    }

    #[test]
    fn imu_metadata_is_validated_and_queued() {
        let mut session = NativeIngestSession::new("phone-a", 2).unwrap();
        let sample = NativeImuSample {
            timestamp_micros: 20_000,
            clock_domain: 1,
            uncertainty_micros: 50,
            acceleration_mps2: NativeVec3 {
                x: 0.0,
                y: 0.0,
                z: 9.81,
            },
            angular_velocity_rps: NativeVec3 {
                x: 0.1,
                y: 0.2,
                z: 0.3,
            },
            gravity_mps2: NativeVec3 {
                x: 0.0,
                y: 0.0,
                z: 9.81,
            },
            has_gravity: 1,
        };
        assert_eq!(session.ingest_imu(sample), NativeStatus::Ok);
        assert_eq!(session.stats().accepted_imu_samples, 1);
        assert!(matches!(
            session.pop_observation(),
            Some(RawObservation::Imu(_))
        ));
    }

    #[test]
    fn depth_buffers_are_copied_into_bounded_rust_owned_queue() {
        let mut session = NativeIngestSession::new("phone-a", 1).unwrap();
        let descriptor = NativeDepthDescriptor {
            timestamp_micros: 30_000,
            clock_domain: 2,
            uncertainty_micros: 100,
            width_px: 2,
            height_px: 2,
            depth_format: NativeDepthFormat::U16Millimeters as u32,
            depth_row_stride_bytes: 4,
            confidence_row_stride_bytes: 2,
            min_depth_meters: 0.1,
            max_depth_meters: 5.0,
            fresh_for_frame: 1,
        };

        for value in 0..6u8 {
            let depth = vec![value; 8];
            let confidence = vec![255 - value; 4];
            assert_eq!(
                session.ingest_depth(descriptor, &depth, Some(&confidence)),
                NativeStatus::Ok
            );
        }

        let stats = session.stats();
        assert_eq!(stats.accepted_depth_frames, 6);
        assert_eq!(stats.queued_depth_frames, MAX_DEPTH_FRAMES);
        assert_eq!(stats.dropped_depth_frames, 2);

        let first = session.pop_depth_frame().unwrap();
        assert_eq!(first.depth_bytes, vec![2; 8]);
        assert_eq!(first.confidence_bytes, Some(vec![253; 4]));
        assert!(first.fresh_for_frame);
    }

    #[test]
    fn derives_depth_range_when_platform_does_not_supply_one() {
        let mut session = NativeIngestSession::new("phone-a", 1).unwrap();
        let descriptor = NativeDepthDescriptor {
            timestamp_micros: 1,
            clock_domain: 2,
            uncertainty_micros: 0,
            width_px: 2,
            height_px: 2,
            depth_format: NativeDepthFormat::F32Meters as u32,
            depth_row_stride_bytes: 8,
            confidence_row_stride_bytes: 2,
            min_depth_meters: 0.0,
            max_depth_meters: 0.0,
            fresh_for_frame: 1,
        };
        let values = [0.0f32, 0.5, 2.0, f32::NAN];
        let mut bytes = Vec::new();
        for value in values {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }

        assert_eq!(
            session.ingest_depth(descriptor, &bytes, None),
            NativeStatus::Ok
        );

        let observation = session
            .observations
            .iter()
            .find_map(|observation| match observation {
                RawObservation::Depth(depth) => Some(depth),
                _ => None,
            })
            .unwrap();
        assert!((observation.payload.min_depth_meters - 0.5).abs() < f32::EPSILON);
        assert!((observation.payload.max_depth_meters - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn rejects_malformed_depth_stride_and_buffer() {
        let mut session = NativeIngestSession::new("phone-a", 1).unwrap();
        let mut descriptor = NativeDepthDescriptor {
            timestamp_micros: 1,
            clock_domain: 2,
            uncertainty_micros: 0,
            width_px: 4,
            height_px: 2,
            depth_format: NativeDepthFormat::F32Meters as u32,
            depth_row_stride_bytes: 8,
            confidence_row_stride_bytes: 4,
            min_depth_meters: 0.0,
            max_depth_meters: 4.0,
            fresh_for_frame: 1,
        };

        assert_eq!(
            session.ingest_depth(descriptor, &[0; 32], None),
            NativeStatus::InvalidBuffer
        );

        descriptor.depth_row_stride_bytes = 16;
        assert_eq!(
            session.ingest_depth(descriptor, &[0; 16], None),
            NativeStatus::InvalidBuffer
        );
    }

    #[test]
    fn ffi_null_handles_fail_without_dereference() {
        let metadata = camera_metadata();
        let status = unsafe {
            scanner_native_ingest_camera_metadata(std::ptr::null_mut(), &metadata)
        };
        assert_eq!(status, NativeStatus::NullHandle);
    }
}
