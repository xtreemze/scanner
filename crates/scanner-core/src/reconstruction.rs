use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::observation::{Pose, Quaternion, Vec3};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelKey {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceSample {
    pub position_camera_meters: Vec3,
    pub confidence: f32,
    pub depth_uncertainty_meters: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfacePoint {
    pub position_session_meters: Vec3,
    pub confidence: f32,
    pub observation_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FusionConfig {
    pub voxel_size_meters: f64,
    pub max_voxels: usize,
    pub max_sample_weight: f64,
}

impl Default for FusionConfig {
    fn default() -> Self {
        Self {
            voxel_size_meters: 0.01,
            max_voxels: 250_000,
            max_sample_weight: 10_000.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FusionConfigError {
    InvalidVoxelSize,
    EmptyCapacity,
    InvalidMaxSampleWeight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleRejection {
    NonFinitePosition,
    InvalidConfidence,
    InvalidUncertainty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IntegrationReport {
    pub accepted_samples: usize,
    pub rejected_samples: usize,
    pub created_voxels: usize,
    pub updated_voxels: usize,
    pub evicted_voxels: usize,
}

#[derive(Debug, Clone)]
struct VoxelState {
    weighted_position_sum: Vec3,
    total_weight: f64,
    confidence_sum: f64,
    observation_count: u32,
    last_update_sequence: u64,
}

#[derive(Debug, Clone)]
pub struct SparseSurfaceVolume {
    config: FusionConfig,
    voxels: BTreeMap<VoxelKey, VoxelState>,
    update_sequence: u64,
}

impl SparseSurfaceVolume {
    pub fn new(config: FusionConfig) -> Result<Self, FusionConfigError> {
        if !config.voxel_size_meters.is_finite() || config.voxel_size_meters <= 0.0 {
            return Err(FusionConfigError::InvalidVoxelSize);
        }
        if config.max_voxels == 0 {
            return Err(FusionConfigError::EmptyCapacity);
        }
        if !config.max_sample_weight.is_finite() || config.max_sample_weight <= 0.0 {
            return Err(FusionConfigError::InvalidMaxSampleWeight);
        }

        Ok(Self {
            config,
            voxels: BTreeMap::new(),
            update_sequence: 0,
        })
    }

    pub fn config(&self) -> &FusionConfig {
        &self.config
    }

    pub fn voxel_count(&self) -> usize {
        self.voxels.len()
    }

    pub fn integrate_frame(
        &mut self,
        camera_pose_session: Pose,
        samples: &[SurfaceSample],
    ) -> IntegrationReport {
        let mut report = IntegrationReport::default();

        for sample in samples {
            let Some(weight) = sample_weight(*sample, self.config.max_sample_weight) else {
                report.rejected_samples += 1;
                continue;
            };

            let position_session = transform_point(camera_pose_session, sample.position_camera_meters);
            if !vec_is_finite(position_session) {
                report.rejected_samples += 1;
                continue;
            }

            let Some(key) = voxel_key(position_session, self.config.voxel_size_meters) else {
                report.rejected_samples += 1;
                continue;
            };

            self.update_sequence = self.update_sequence.saturating_add(1);
            let sequence = self.update_sequence;

            match self.voxels.get_mut(&key) {
                Some(voxel) => {
                    voxel.weighted_position_sum = add_vec(
                        voxel.weighted_position_sum,
                        scale_vec(position_session, weight),
                    );
                    voxel.total_weight += weight;
                    voxel.confidence_sum += f64::from(sample.confidence);
                    voxel.observation_count = voxel.observation_count.saturating_add(1);
                    voxel.last_update_sequence = sequence;
                    report.updated_voxels += 1;
                }
                None => {
                    self.voxels.insert(
                        key,
                        VoxelState {
                            weighted_position_sum: scale_vec(position_session, weight),
                            total_weight: weight,
                            confidence_sum: f64::from(sample.confidence),
                            observation_count: 1,
                            last_update_sequence: sequence,
                        },
                    );
                    report.created_voxels += 1;
                }
            }

            report.accepted_samples += 1;
        }

        while self.voxels.len() > self.config.max_voxels {
            if let Some(key) = self.lowest_value_voxel_key() {
                self.voxels.remove(&key);
                report.evicted_voxels += 1;
            } else {
                break;
            }
        }

        report
    }

    pub fn extract_points(&self, min_confidence: f32) -> Vec<SurfacePoint> {
        let threshold = min_confidence.clamp(0.0, 1.0);

        self.voxels
            .values()
            .filter_map(|voxel| {
                if voxel.total_weight <= 0.0 || voxel.observation_count == 0 {
                    return None;
                }

                let confidence =
                    (voxel.confidence_sum / f64::from(voxel.observation_count)).clamp(0.0, 1.0)
                        as f32;
                if confidence < threshold {
                    return None;
                }

                Some(SurfacePoint {
                    position_session_meters: scale_vec(
                        voxel.weighted_position_sum,
                        1.0 / voxel.total_weight,
                    ),
                    confidence,
                    observation_count: voxel.observation_count,
                })
            })
            .collect()
    }

    fn lowest_value_voxel_key(&self) -> Option<VoxelKey> {
        self.voxels
            .iter()
            .min_by(|(key_a, a), (key_b, b)| {
                a.total_weight
                    .total_cmp(&b.total_weight)
                    .then_with(|| a.last_update_sequence.cmp(&b.last_update_sequence))
                    .then_with(|| key_a.cmp(key_b))
            })
            .map(|(key, _)| *key)
    }
}

fn sample_weight(sample: SurfaceSample, max_weight: f64) -> Option<f64> {
    if !vec_is_finite(sample.position_camera_meters) {
        return None;
    }
    if !sample.confidence.is_finite() || !(0.0..=1.0).contains(&sample.confidence) {
        return None;
    }
    if !sample.depth_uncertainty_meters.is_finite() || sample.depth_uncertainty_meters <= 0.0 {
        return None;
    }

    let variance = f64::from(sample.depth_uncertainty_meters).powi(2).max(1e-8);
    let weight = (f64::from(sample.confidence) / variance).min(max_weight);
    if weight.is_finite() && weight > 0.0 {
        Some(weight)
    } else {
        None
    }
}

fn voxel_key(point: Vec3, voxel_size: f64) -> Option<VoxelKey> {
    fn component(value: f64, voxel_size: f64) -> Option<i32> {
        let quantized = (value / voxel_size).floor();
        if !quantized.is_finite() || quantized < f64::from(i32::MIN) || quantized > f64::from(i32::MAX) {
            return None;
        }
        Some(quantized as i32)
    }

    Some(VoxelKey {
        x: component(point.x, voxel_size)?,
        y: component(point.y, voxel_size)?,
        z: component(point.z, voxel_size)?,
    })
}

fn transform_point(pose: Pose, point: Vec3) -> Vec3 {
    add_vec(pose.position_meters, rotate_vec(pose.orientation, point))
}

fn rotate_vec(q: Quaternion, v: Vec3) -> Vec3 {
    let Some(q) = normalized_quaternion(q) else {
        return Vec3 {
            x: f64::NAN,
            y: f64::NAN,
            z: f64::NAN,
        };
    };

    let u = Vec3 {
        x: q.x,
        y: q.y,
        z: q.z,
    };
    let s = q.w;
    let uv = dot(u, v);
    let uu = dot(u, u);
    let cross = cross(u, v);

    add_vec(
        add_vec(scale_vec(u, 2.0 * uv), scale_vec(v, s * s - uu)),
        scale_vec(cross, 2.0 * s),
    )
}

fn normalized_quaternion(q: Quaternion) -> Option<Quaternion> {
    if !q.x.is_finite() || !q.y.is_finite() || !q.z.is_finite() || !q.w.is_finite() {
        return None;
    }
    let norm = (q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w).sqrt();
    if norm <= 1e-12 || !norm.is_finite() {
        return None;
    }

    Some(Quaternion {
        x: q.x / norm,
        y: q.y / norm,
        z: q.z / norm,
        w: q.w / norm,
    })
}

fn vec_is_finite(v: Vec3) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.z.is_finite()
}

fn add_vec(a: Vec3, b: Vec3) -> Vec3 {
    Vec3 {
        x: a.x + b.x,
        y: a.y + b.y,
        z: a.z + b.z,
    }
}

fn scale_vec(v: Vec3, scale: f64) -> Vec3 {
    Vec3 {
        x: v.x * scale,
        y: v.y * scale,
        z: v.z * scale,
    }
}

fn dot(a: Vec3, b: Vec3) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    Vec3 {
        x: a.y * b.z - a.z * b.y,
        y: a.z * b.x - a.x * b.z,
        z: a.x * b.y - a.y * b.x,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity_pose() -> Pose {
        Pose {
            position_meters: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            orientation: Quaternion {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        }
    }

    fn sample(x: f64, confidence: f32, uncertainty: f32) -> SurfaceSample {
        SurfaceSample {
            position_camera_meters: Vec3 { x, y: 0.0, z: 1.0 },
            confidence,
            depth_uncertainty_meters: uncertainty,
        }
    }

    #[test]
    fn repeated_observations_converge_inside_one_voxel() {
        let mut volume = SparseSurfaceVolume::new(FusionConfig {
            voxel_size_meters: 0.1,
            max_voxels: 10,
            max_sample_weight: 10_000.0,
        })
        .unwrap();

        volume.integrate_frame(identity_pose(), &[sample(0.011, 1.0, 0.01)]);
        volume.integrate_frame(identity_pose(), &[sample(0.019, 1.0, 0.01)]);

        let points = volume.extract_points(0.0);
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].observation_count, 2);
        assert!((points[0].position_session_meters.x - 0.015).abs() < 1e-9);
    }

    #[test]
    fn lower_uncertainty_observation_has_more_influence() {
        let mut volume = SparseSurfaceVolume::new(FusionConfig {
            voxel_size_meters: 1.0,
            max_voxels: 10,
            max_sample_weight: 100_000.0,
        })
        .unwrap();

        volume.integrate_frame(
            identity_pose(),
            &[
                sample(0.1, 1.0, 0.01),
                sample(0.9, 1.0, 0.10),
            ],
        );

        let point = volume.extract_points(0.0)[0];
        assert!(point.position_session_meters.x < 0.2);
    }

    #[test]
    fn camera_pose_places_points_in_session_world() {
        let mut volume = SparseSurfaceVolume::new(FusionConfig::default()).unwrap();
        let pose = Pose {
            position_meters: Vec3 {
                x: 2.0,
                y: 3.0,
                z: 4.0,
            },
            ..identity_pose()
        };

        volume.integrate_frame(pose, &[sample(0.0, 1.0, 0.01)]);
        let point = volume.extract_points(0.0)[0];

        assert_eq!(
            point.position_session_meters,
            Vec3 {
                x: 2.0,
                y: 3.0,
                z: 5.0,
            }
        );
    }

    #[test]
    fn invalid_samples_are_rejected_without_poisoning_volume() {
        let mut volume = SparseSurfaceVolume::new(FusionConfig::default()).unwrap();
        let invalid = SurfaceSample {
            position_camera_meters: Vec3 {
                x: f64::NAN,
                y: 0.0,
                z: 1.0,
            },
            confidence: 1.0,
            depth_uncertainty_meters: 0.01,
        };

        let report = volume.integrate_frame(identity_pose(), &[invalid]);
        assert_eq!(report.accepted_samples, 0);
        assert_eq!(report.rejected_samples, 1);
        assert_eq!(volume.voxel_count(), 0);
    }

    #[test]
    fn capacity_is_bounded_and_weakest_voxel_is_evicted() {
        let mut volume = SparseSurfaceVolume::new(FusionConfig {
            voxel_size_meters: 0.1,
            max_voxels: 2,
            max_sample_weight: 10_000.0,
        })
        .unwrap();

        let report = volume.integrate_frame(
            identity_pose(),
            &[
                sample(0.0, 1.0, 0.01),
                sample(0.2, 0.1, 0.10),
                sample(0.4, 0.9, 0.02),
            ],
        );

        assert_eq!(volume.voxel_count(), 2);
        assert_eq!(report.evicted_voxels, 1);

        let points = volume.extract_points(0.5);
        assert_eq!(points.len(), 2);
    }
}
