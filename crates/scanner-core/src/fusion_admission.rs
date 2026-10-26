//! Decides which retained depth frames are independent, metric fusion evidence.
//!
//! This is a *fusion admission* guard, not a raw-capture filter. Reprojected
//! depth, relative monocular predictions, and rejected timestamps remain
//! eligible for archival, debugging, and presentation. The fusion worker
//! should call this gate before integrating a frame, using one gate per session.
//! It does not certify camera calibration, pose quality, or geometric accuracy.

use std::collections::BTreeMap;

use crate::native_ingest::NativeDepthFrame;
use crate::observation::{ClockDomain, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthScaleEvidence {
    /// ARKit/ARCore or another platform supplies metrically scaled depth.
    PlatformMetric,
    /// Software depth was aligned to independently established metric scale.
    AlignedSoftwareMetric,
    /// Relative monocular depth lacks a defensible metric scale.
    UncalibratedRelative,
}

#[derive(Debug, Clone, Copy)]
pub struct DepthFusionCandidate<'a> {
    pub device_id: &'a str,
    pub epoch: u64,
    pub captured_at: Timestamp,
    /// False for an ARCore raw-depth frame reprojected from prior evidence.
    pub fresh_measurement: bool,
    pub scale_evidence: DepthScaleEvidence,
}

impl<'a> DepthFusionCandidate<'a> {
    pub fn from_native(frame: &'a NativeDepthFrame, scale_evidence: DepthScaleEvidence) -> Self {
        Self {
            device_id: &frame.device_id,
            epoch: frame.epoch,
            captured_at: frame.captured_at,
            fresh_measurement: frame.fresh_for_frame,
            scale_evidence,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthFusionDecision {
    Integrate,
    InvalidDevice,
    ReprojectedOrReused,
    UncalibratedScale,
    UncertainTimestamp,
    StaleEpoch,
    ClockDomainChanged,
    DuplicateOrOutOfOrder,
    DeviceCapacityReached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DepthAdmissionConfig {
    /// Maximum uncertainty in the timestamp's own clock domain. This does
    /// not replace inter-device synchronization and pose uncertainty checks.
    pub max_timestamp_uncertainty_micros: u32,
    /// Bounds the peer-index memory footprint.
    pub max_devices: usize,
}

impl Default for DepthAdmissionConfig {
    fn default() -> Self {
        Self {
            max_timestamp_uncertainty_micros: 50_000,
            max_devices: 16,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct LastAccepted {
    epoch: u64,
    clock_domain: ClockDomain,
    timestamp_micros: u64,
}

#[derive(Debug, Default)]
pub struct DepthFusionGate {
    config: DepthAdmissionConfig,
    last_accepted: BTreeMap<String, LastAccepted>,
}

impl DepthFusionGate {
    pub fn new(config: DepthAdmissionConfig) -> Option<Self> {
        (config.max_devices > 0).then(|| Self {
            config,
            last_accepted: BTreeMap::new(),
        })
    }

    /// Call only when the accepted frame is actually submitted for fusion.
    /// Rejected observations never advance the watermark.
    pub fn admit(&mut self, frame: DepthFusionCandidate<'_>) -> DepthFusionDecision {
        if frame.device_id.trim().is_empty() {
            return DepthFusionDecision::InvalidDevice;
        }
        if !frame.fresh_measurement {
            return DepthFusionDecision::ReprojectedOrReused;
        }
        if frame.scale_evidence == DepthScaleEvidence::UncalibratedRelative {
            return DepthFusionDecision::UncalibratedScale;
        }
        if frame.captured_at.uncertainty_micros > self.config.max_timestamp_uncertainty_micros {
            return DepthFusionDecision::UncertainTimestamp;
        }

        if let Some(last) = self.last_accepted.get(frame.device_id) {
            if frame.epoch < last.epoch {
                return DepthFusionDecision::StaleEpoch;
            }
            if frame.epoch == last.epoch {
                if frame.captured_at.clock_domain != last.clock_domain {
                    return DepthFusionDecision::ClockDomainChanged;
                }
                if frame.captured_at.micros <= last.timestamp_micros {
                    return DepthFusionDecision::DuplicateOrOutOfOrder;
                }
            }
        } else if self.last_accepted.len() >= self.config.max_devices {
            return DepthFusionDecision::DeviceCapacityReached;
        }

        self.last_accepted.insert(
            frame.device_id.to_owned(),
            LastAccepted {
                epoch: frame.epoch,
                clock_domain: frame.captured_at.clock_domain,
                timestamp_micros: frame.captured_at.micros,
            },
        );
        DepthFusionDecision::Integrate
    }

    pub fn admit_native(
        &mut self,
        frame: &NativeDepthFrame,
        scale_evidence: DepthScaleEvidence,
    ) -> DepthFusionDecision {
        self.admit(DepthFusionCandidate::from_native(frame, scale_evidence))
    }

    /// Explicitly remove an old peer when a session drops it.
    pub fn forget_device(&mut self, device_id: &str) {
        self.last_accepted.remove(device_id);
    }

    pub fn tracked_devices(&self) -> usize {
        self.last_accepted.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate<'a>(device: &'a str, epoch: u64, micros: u64) -> DepthFusionCandidate<'a> {
        DepthFusionCandidate {
            device_id: device,
            epoch,
            captured_at: Timestamp {
                micros,
                clock_domain: ClockDomain::CameraSensor,
                uncertainty_micros: 100,
            },
            fresh_measurement: true,
            scale_evidence: DepthScaleEvidence::PlatformMetric,
        }
    }

    #[test]
    fn fresh_frames_integrate_but_reprojection_and_duplicates_do_not() {
        let mut gate = DepthFusionGate::default();
        assert_eq!(gate.admit(candidate("phone", 1, 100)), DepthFusionDecision::Integrate);
        let mut reused = candidate("phone", 1, 200);
        reused.fresh_measurement = false;
        assert_eq!(gate.admit(reused), DepthFusionDecision::ReprojectedOrReused);
        assert_eq!(
            gate.admit(candidate("phone", 1, 100)),
            DepthFusionDecision::DuplicateOrOutOfOrder
        );
        assert_eq!(
            gate.admit(candidate("phone", 1, 50)),
            DepthFusionDecision::DuplicateOrOutOfOrder
        );
        assert_eq!(gate.admit(candidate("phone", 1, 200)), DepthFusionDecision::Integrate);
    }

    #[test]
    fn rejects_relative_depth_without_metric_alignment() {
        let mut gate = DepthFusionGate::default();
        let mut relative = candidate("phone", 0, 100);
        relative.scale_evidence = DepthScaleEvidence::UncalibratedRelative;
        assert_eq!(gate.admit(relative), DepthFusionDecision::UncalibratedScale);
        relative.scale_evidence = DepthScaleEvidence::AlignedSoftwareMetric;
        assert_eq!(gate.admit(relative), DepthFusionDecision::Integrate);
    }

    #[test]
    fn uncertain_timestamps_and_clock_switches_do_not_poison_watermark() {
        let mut gate = DepthFusionGate::default();
        let mut uncertain = candidate("a", 1, 100);
        uncertain.captured_at.uncertainty_micros = 60_000;
        assert_eq!(gate.admit(uncertain), DepthFusionDecision::UncertainTimestamp);
        assert_eq!(gate.admit(candidate("a", 1, 100)), DepthFusionDecision::Integrate);
        let mut changed = candidate("a", 1, 101);
        changed.captured_at.clock_domain = ClockDomain::DeviceMonotonic;
        assert_eq!(gate.admit(changed), DepthFusionDecision::ClockDomainChanged);
        assert_eq!(gate.admit(candidate("a", 1, 101)), DepthFusionDecision::Integrate);
    }

    #[test]
    fn new_epoch_resets_watermark_but_old_epochs_are_rejected() {
        let mut gate = DepthFusionGate::default();
        assert_eq!(gate.admit(candidate("a", 4, 800)), DepthFusionDecision::Integrate);
        assert_eq!(gate.admit(candidate("a", 5, 10)), DepthFusionDecision::Integrate);
        assert_eq!(gate.admit(candidate("a", 4, 900)), DepthFusionDecision::StaleEpoch);
        assert_eq!(gate.admit(candidate("a", 5, 11)), DepthFusionDecision::Integrate);
    }

    #[test]
    fn distinct_peers_are_independent_and_capacity_is_bounded() {
        let mut gate = DepthFusionGate::new(DepthAdmissionConfig {
            max_timestamp_uncertainty_micros: 50_000,
            max_devices: 2,
        }).unwrap();
        assert_eq!(gate.admit(candidate("a", 1, 100)), DepthFusionDecision::Integrate);
        assert_eq!(gate.admit(candidate("b", 1, 100)), DepthFusionDecision::Integrate);
        assert_eq!(
            gate.admit(candidate("c", 1, 100)),
            DepthFusionDecision::DeviceCapacityReached
        );
        gate.forget_device("a");
        assert_eq!(gate.admit(candidate("c", 1, 100)), DepthFusionDecision::Integrate);
        assert_eq!(gate.tracked_devices(), 2);
        assert!(DepthFusionGate::new(DepthAdmissionConfig {
            max_devices: 0,
            ..DepthAdmissionConfig::default()
        }).is_none());
    }

    #[test]
    fn empty_device_id_is_rejected() {
        assert_eq!(
            DepthFusionGate::default().admit(candidate(" ", 1, 10)),
            DepthFusionDecision::InvalidDevice
        );
    }
}
