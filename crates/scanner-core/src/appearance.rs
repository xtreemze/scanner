use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::observation::Vec3;

const DEFAULT_AZIMUTH_BINS: usize = 8;
const DEFAULT_ELEVATION_BINS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentSample {
    pub direction_session: Vec3,
    pub confidence: f32,
    pub exposure_value: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvironmentSampleError {
    InvalidDirection,
    InvalidConfidence,
    InvalidExposure,
}

#[derive(Debug, Clone)]
pub struct EnvironmentCoverage {
    occupied_bins: BTreeSet<usize>,
    azimuth_bins: usize,
    elevation_bins: usize,
}

impl Default for EnvironmentCoverage {
    fn default() -> Self {
        Self::new(DEFAULT_AZIMUTH_BINS, DEFAULT_ELEVATION_BINS)
    }
}

impl EnvironmentCoverage {
    pub fn new(azimuth_bins: usize, elevation_bins: usize) -> Self {
        Self {
            occupied_bins: BTreeSet::new(),
            azimuth_bins: azimuth_bins.max(1),
            elevation_bins: elevation_bins.max(1),
        }
    }

    pub fn observe(&mut self, sample: EnvironmentSample) -> Result<(), EnvironmentSampleError> {
        if !(0.0..=1.0).contains(&sample.confidence) || !sample.confidence.is_finite() {
            return Err(EnvironmentSampleError::InvalidConfidence);
        }
        if !sample.exposure_value.is_finite() {
            return Err(EnvironmentSampleError::InvalidExposure);
        }

        let direction = normalize(sample.direction_session)
            .ok_or(EnvironmentSampleError::InvalidDirection)?;
        let azimuth = direction.z.atan2(direction.x);
        let elevation = direction.y.clamp(-1.0, 1.0).asin();

        let azimuth_unit = (azimuth + std::f64::consts::PI)
            / (2.0 * std::f64::consts::PI);
        let elevation_unit = (elevation + std::f64::consts::FRAC_PI_2)
            / std::f64::consts::PI;

        let azimuth_index = ((azimuth_unit * self.azimuth_bins as f64).floor() as usize)
            .min(self.azimuth_bins - 1);
        let elevation_index =
            ((elevation_unit * self.elevation_bins as f64).floor() as usize)
                .min(self.elevation_bins - 1);

        self.occupied_bins
            .insert(elevation_index * self.azimuth_bins + azimuth_index);
        Ok(())
    }

    pub fn coverage_fraction(&self) -> f32 {
        self.occupied_bins.len() as f32
            / (self.azimuth_bins * self.elevation_bins) as f32
    }

    pub fn occupied_bin_count(&self) -> usize {
        self.occupied_bins.len()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlledIlluminationPair {
    pub ambient_frame_id: String,
    pub illuminated_frame_id: String,
    pub illumination_observation_id: String,
    pub capture_device_id: String,
    pub emitter_device_id: String,
    pub time_delta_micros: u64,
    pub camera_translation_delta_meters: f64,
    pub camera_rotation_delta_degrees: f64,
    pub exposure_delta_ev: f32,
    pub emitter_position_session: Option<Vec3>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingThresholds {
    pub max_time_delta_micros: u64,
    pub max_translation_delta_meters: f64,
    pub max_rotation_delta_degrees: f64,
    pub max_exposure_delta_ev: f32,
}

impl Default for PairingThresholds {
    fn default() -> Self {
        Self {
            max_time_delta_micros: 250_000,
            max_translation_delta_meters: 0.015,
            max_rotation_delta_degrees: 1.5,
            max_exposure_delta_ev: 0.25,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingError {
    MissingIdentity,
    TimeDeltaTooLarge,
    TranslationDeltaTooLarge,
    RotationDeltaTooLarge,
    ExposureDeltaTooLarge,
    InvalidMeasurement,
}

impl ControlledIlluminationPair {
    pub fn validate(&self, thresholds: PairingThresholds) -> Result<(), PairingError> {
        if self.ambient_frame_id.is_empty()
            || self.illuminated_frame_id.is_empty()
            || self.illumination_observation_id.is_empty()
            || self.capture_device_id.is_empty()
            || self.emitter_device_id.is_empty()
        {
            return Err(PairingError::MissingIdentity);
        }

        if !self.camera_translation_delta_meters.is_finite()
            || !self.camera_rotation_delta_degrees.is_finite()
            || !self.exposure_delta_ev.is_finite()
            || self.camera_translation_delta_meters < 0.0
            || self.camera_rotation_delta_degrees < 0.0
            || self
                .emitter_position_session
                .is_some_and(|position| !vec_is_finite(position))
        {
            return Err(PairingError::InvalidMeasurement);
        }

        if self.time_delta_micros > thresholds.max_time_delta_micros {
            return Err(PairingError::TimeDeltaTooLarge);
        }
        if self.camera_translation_delta_meters > thresholds.max_translation_delta_meters {
            return Err(PairingError::TranslationDeltaTooLarge);
        }
        if self.camera_rotation_delta_degrees > thresholds.max_rotation_delta_degrees {
            return Err(PairingError::RotationDeltaTooLarge);
        }
        if self.exposure_delta_ev.abs() > thresholds.max_exposure_delta_ev {
            return Err(PairingError::ExposureDeltaTooLarge);
        }
        Ok(())
    }

    pub fn has_known_emitter_pose(&self) -> bool {
        self.emitter_position_session.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PbrMaterialEstimate {
    pub base_color_linear_rgba: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub confidence: f32,
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaterialEstimateError {
    InvalidBaseColor,
    InvalidMetallic,
    InvalidRoughness,
    InvalidConfidence,
    MissingEvidence,
}

impl PbrMaterialEstimate {
    pub fn validate(&self) -> Result<(), MaterialEstimateError> {
        if !self
            .base_color_linear_rgba
            .iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        {
            return Err(MaterialEstimateError::InvalidBaseColor);
        }
        if !self.metallic.is_finite() || !(0.0..=1.0).contains(&self.metallic) {
            return Err(MaterialEstimateError::InvalidMetallic);
        }
        if !self.roughness.is_finite() || !(0.0..=1.0).contains(&self.roughness) {
            return Err(MaterialEstimateError::InvalidRoughness);
        }
        if !self.confidence.is_finite() || !(0.0..=1.0).contains(&self.confidence) {
            return Err(MaterialEstimateError::InvalidConfidence);
        }
        if self.evidence_ids.is_empty() {
            return Err(MaterialEstimateError::MissingEvidence);
        }
        Ok(())
    }
}

fn normalize(v: Vec3) -> Option<Vec3> {
    if !vec_is_finite(v) {
        return None;
    }
    let magnitude = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
    if magnitude <= 1e-12 {
        return None;
    }
    Some(Vec3 {
        x: v.x / magnitude,
        y: v.y / magnitude,
        z: v.z / magnitude,
    })
}

fn vec_is_finite(v: Vec3) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.z.is_finite()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_coverage_accumulates_directional_bins() {
        let mut coverage = EnvironmentCoverage::new(4, 2);
        coverage
            .observe(EnvironmentSample {
                direction_session: Vec3 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                },
                confidence: 1.0,
                exposure_value: 0.0,
            })
            .unwrap();
        coverage
            .observe(EnvironmentSample {
                direction_session: Vec3 {
                    x: -1.0,
                    y: 0.0,
                    z: 0.0,
                },
                confidence: 1.0,
                exposure_value: 1.0,
            })
            .unwrap();

        assert_eq!(coverage.occupied_bin_count(), 2);
        assert_eq!(coverage.coverage_fraction(), 0.25);
    }

    #[test]
    fn invalid_environment_direction_is_rejected() {
        let mut coverage = EnvironmentCoverage::default();
        assert_eq!(
            coverage.observe(EnvironmentSample {
                direction_session: Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                confidence: 1.0,
                exposure_value: 0.0,
            }),
            Err(EnvironmentSampleError::InvalidDirection)
        );
    }

    fn pair() -> ControlledIlluminationPair {
        ControlledIlluminationPair {
            ambient_frame_id: "ambient".into(),
            illuminated_frame_id: "flash".into(),
            illumination_observation_id: "illumination".into(),
            capture_device_id: "camera".into(),
            emitter_device_id: "light".into(),
            time_delta_micros: 20_000,
            camera_translation_delta_meters: 0.002,
            camera_rotation_delta_degrees: 0.2,
            exposure_delta_ev: 0.1,
            emitter_position_session: Some(Vec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            }),
        }
    }

    #[test]
    fn controlled_pair_accepts_stable_capture() {
        let pair = pair();
        assert_eq!(pair.validate(PairingThresholds::default()), Ok(()));
        assert!(pair.has_known_emitter_pose());
    }

    #[test]
    fn controlled_pair_rejects_pose_change() {
        let mut pair = pair();
        pair.camera_translation_delta_meters = 0.1;
        assert_eq!(
            pair.validate(PairingThresholds::default()),
            Err(PairingError::TranslationDeltaTooLarge)
        );
    }

    #[test]
    fn controlled_pair_rejects_exposure_mismatch() {
        let mut pair = pair();
        pair.exposure_delta_ev = 1.0;
        assert_eq!(
            pair.validate(PairingThresholds::default()),
            Err(PairingError::ExposureDeltaTooLarge)
        );
    }

    #[test]
    fn material_estimate_requires_bounded_values_and_evidence() {
        let estimate = PbrMaterialEstimate {
            base_color_linear_rgba: [0.2, 0.3, 0.4, 1.0],
            metallic: 0.0,
            roughness: 0.7,
            confidence: 0.6,
            evidence_ids: vec!["pair-a".into()],
        };
        assert_eq!(estimate.validate(), Ok(()));
    }

    #[test]
    fn material_estimate_without_evidence_is_invalid() {
        let estimate = PbrMaterialEstimate {
            base_color_linear_rgba: [0.2, 0.3, 0.4, 1.0],
            metallic: 0.0,
            roughness: 0.7,
            confidence: 0.6,
            evidence_ids: Vec::new(),
        };
        assert_eq!(
            estimate.validate(),
            Err(MaterialEstimateError::MissingEvidence)
        );
    }
}
