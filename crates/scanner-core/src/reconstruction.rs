use std::collections::{BTreeMap, BTreeSet};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PreviewMesh {
    pub vertices_session_meters: Vec<Vec3>,
    pub triangle_indices: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaycastHit {
    pub voxel: VoxelKey,
    pub position_session_meters: Vec3,
    pub distance_meters: f64,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ReconstructionFrame {
    SessionWorld,
    ObjectLocal {
        object_id: String,
        object_pose_session: Pose,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservationProvenance {
    pub observation_id: String,
    pub device_id: String,
    pub session_timestamp_micros: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VoxelEvidence {
    pub observation_ids: Vec<String>,
    pub device_ids: Vec<String>,
    pub first_session_timestamp_micros: Option<u64>,
    pub last_session_timestamp_micros: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconstructionFrameError {
    NotObjectLocal,
    ObjectIdMismatch,
    InvalidPose,
}

const MAX_PROVENANCE_IDS_PER_VOXEL: usize = 8;

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelCheckpoint {
    pub key: VoxelKey,
    pub weighted_position_sum: Vec3,
    pub total_weight: f64,
    pub confidence_sum: f64,
    pub observation_count: u32,
    pub last_update_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SparseSurfaceCheckpoint {
    pub config: FusionConfig,
    pub voxels: Vec<VoxelCheckpoint>,
    pub update_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelEvidenceCheckpoint {
    pub key: VoxelKey,
    pub evidence: VoxelEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconstructionCheckpoint {
    pub frame: ReconstructionFrame,
    pub surface: SparseSurfaceCheckpoint,
    pub evidence: Vec<VoxelEvidenceCheckpoint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconstructionRestoreError {
    InvalidConfig(FusionConfigError),
    TooManyVoxels,
    DuplicateVoxel,
    InvalidVoxelState,
    InvalidFrame,
    DuplicateEvidence,
    EvidenceForMissingVoxel,
    InvalidEvidence,
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
    pub fn checkpoint(&self) -> SparseSurfaceCheckpoint {
        SparseSurfaceCheckpoint {
            config: self.config.clone(),
            voxels: self
                .voxels
                .iter()
                .map(|(key, voxel)| VoxelCheckpoint {
                    key: *key,
                    weighted_position_sum: voxel.weighted_position_sum,
                    total_weight: voxel.total_weight,
                    confidence_sum: voxel.confidence_sum,
                    observation_count: voxel.observation_count,
                    last_update_sequence: voxel.last_update_sequence,
                })
                .collect(),
            update_sequence: self.update_sequence,
        }
    }

    pub fn from_checkpoint(
        checkpoint: SparseSurfaceCheckpoint,
    ) -> Result<Self, ReconstructionRestoreError> {
        let mut volume =
            SparseSurfaceVolume::new(checkpoint.config.clone())
                .map_err(ReconstructionRestoreError::InvalidConfig)?;

        if checkpoint.voxels.len() > checkpoint.config.max_voxels {
            return Err(ReconstructionRestoreError::TooManyVoxels);
        }

        for saved in checkpoint.voxels {
            let valid = vec_is_finite(saved.weighted_position_sum)
                && saved.total_weight.is_finite()
                && saved.total_weight > 0.0
                && saved.confidence_sum.is_finite()
                && saved.confidence_sum >= 0.0
                && saved.observation_count > 0
                && saved.confidence_sum
                    <= f64::from(saved.observation_count) + f64::EPSILON
                && saved.last_update_sequence <= checkpoint.update_sequence;
            if !valid {
                return Err(ReconstructionRestoreError::InvalidVoxelState);
            }

            if volume
                .voxels
                .insert(
                    saved.key,
                    VoxelState {
                        weighted_position_sum: saved.weighted_position_sum,
                        total_weight: saved.total_weight,
                        confidence_sum: saved.confidence_sum,
                        observation_count: saved.observation_count,
                        last_update_sequence: saved.last_update_sequence,
                    },
                )
                .is_some()
            {
                return Err(ReconstructionRestoreError::DuplicateVoxel);
            }
        }

        volume.update_sequence = checkpoint.update_sequence;
        Ok(volume)
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

                let confidence = voxel_confidence(voxel);
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

    pub fn extract_preview_mesh(&self, min_confidence: f32) -> PreviewMesh {
        let threshold = min_confidence.clamp(0.0, 1.0);
        let active = self
            .voxels
            .iter()
            .filter_map(|(key, voxel)| {
                (voxel_confidence(voxel) >= threshold).then_some(*key)
            })
            .collect::<BTreeSet<_>>();

        let mut mesh = PreviewMesh::default();
        let faces: [(VoxelKey, [[f64; 3]; 4]); 6] = [
            (
                VoxelKey { x: -1, y: 0, z: 0 },
                [[0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 1.0], [0.0, 1.0, 0.0]],
            ),
            (
                VoxelKey { x: 1, y: 0, z: 0 },
                [[1.0, 0.0, 1.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [1.0, 1.0, 1.0]],
            ),
            (
                VoxelKey { x: 0, y: -1, z: 0 },
                [[0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 1.0]],
            ),
            (
                VoxelKey { x: 0, y: 1, z: 0 },
                [[0.0, 1.0, 0.0], [0.0, 1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, 0.0]],
            ),
            (
                VoxelKey { x: 0, y: 0, z: -1 },
                [[1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0]],
            ),
            (
                VoxelKey { x: 0, y: 0, z: 1 },
                [[0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 1.0], [0.0, 1.0, 1.0]],
            ),
        ];

        for key in &active {
            for (delta, corners) in faces {
                let Some(neighbor) = offset_key(*key, delta) else {
                    continue;
                };
                if active.contains(&neighbor) {
                    continue;
                }
                if mesh.vertices_session_meters.len() > (u32::MAX as usize).saturating_sub(4) {
                    return mesh;
                }

                let base = mesh.vertices_session_meters.len() as u32;
                for corner in corners {
                    mesh.vertices_session_meters.push(Vec3 {
                        x: (f64::from(key.x) + corner[0]) * self.config.voxel_size_meters,
                        y: (f64::from(key.y) + corner[1]) * self.config.voxel_size_meters,
                        z: (f64::from(key.z) + corner[2]) * self.config.voxel_size_meters,
                    });
                }
                mesh.triangle_indices.extend_from_slice(&[
                    base,
                    base + 1,
                    base + 2,
                    base,
                    base + 2,
                    base + 3,
                ]);
            }
        }

        mesh
    }

    pub fn raycast_surface(
        &self,
        origin_session_meters: Vec3,
        direction_session: Vec3,
        min_confidence: f32,
    ) -> Option<RaycastHit> {
        if !vec_is_finite(origin_session_meters) {
            return None;
        }
        let direction = normalize_vec(direction_session)?;
        let threshold = min_confidence.clamp(0.0, 1.0);
        let size = self.config.voxel_size_meters;

        self.voxels
            .iter()
            .filter_map(|(key, voxel)| {
                let confidence = voxel_confidence(voxel);
                if confidence < threshold {
                    return None;
                }

                let min = Vec3 {
                    x: f64::from(key.x) * size,
                    y: f64::from(key.y) * size,
                    z: f64::from(key.z) * size,
                };
                let max = Vec3 {
                    x: min.x + size,
                    y: min.y + size,
                    z: min.z + size,
                };
                let distance = ray_aabb_distance(origin_session_meters, direction, min, max)?;
                Some(RaycastHit {
                    voxel: *key,
                    position_session_meters: add_vec(
                        origin_session_meters,
                        scale_vec(direction, distance),
                    ),
                    distance_meters: distance,
                    confidence,
                })
            })
            .min_by(|a, b| {
                a.distance_meters
                    .total_cmp(&b.distance_meters)
                    .then_with(|| a.voxel.cmp(&b.voxel))
            })
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

#[derive(Debug, Clone)]
pub struct ReconstructionVolume {
    frame: ReconstructionFrame,
    surface: SparseSurfaceVolume,
    evidence: BTreeMap<VoxelKey, VoxelEvidence>,
}

impl ReconstructionVolume {
    pub fn new(
        config: FusionConfig,
        frame: ReconstructionFrame,
    ) -> Result<Self, FusionConfigError> {
        if let ReconstructionFrame::ObjectLocal {
            object_pose_session, ..
        } = &frame
        {
            if !pose_is_finite(*object_pose_session) {
                return Err(FusionConfigError::InvalidVoxelSize);
            }
        }

        Ok(Self {
            frame,
            surface: SparseSurfaceVolume::new(config)?,
            evidence: BTreeMap::new(),
        })
    }

    pub fn frame(&self) -> &ReconstructionFrame {
        &self.frame
    }
    pub fn checkpoint(&self) -> ReconstructionCheckpoint {
        ReconstructionCheckpoint {
            frame: self.frame.clone(),
            surface: self.surface.checkpoint(),
            evidence: self
                .evidence
                .iter()
                .map(|(key, evidence)| VoxelEvidenceCheckpoint {
                    key: *key,
                    evidence: evidence.clone(),
                })
                .collect(),
        }
    }

    pub fn from_checkpoint(
        checkpoint: ReconstructionCheckpoint,
    ) -> Result<Self, ReconstructionRestoreError> {
        if let ReconstructionFrame::ObjectLocal {
            object_pose_session, ..
        } = &checkpoint.frame
        {
            if !pose_is_finite(*object_pose_session) {
                return Err(ReconstructionRestoreError::InvalidFrame);
            }
        }

        let surface = SparseSurfaceVolume::from_checkpoint(checkpoint.surface)?;
        let mut evidence = BTreeMap::new();

        for saved in checkpoint.evidence {
            if !surface.voxels.contains_key(&saved.key) {
                return Err(ReconstructionRestoreError::EvidenceForMissingVoxel);
            }
            if saved.evidence.observation_ids.len() > MAX_PROVENANCE_IDS_PER_VOXEL
                || saved.evidence.device_ids.len() > MAX_PROVENANCE_IDS_PER_VOXEL
                || matches!(
                    (
                        saved.evidence.first_session_timestamp_micros,
                        saved.evidence.last_session_timestamp_micros,
                    ),
                    (Some(first), Some(last)) if first > last
                )
            {
                return Err(ReconstructionRestoreError::InvalidEvidence);
            }
            if evidence.insert(saved.key, saved.evidence).is_some() {
                return Err(ReconstructionRestoreError::DuplicateEvidence);
            }
        }

        Ok(Self {
            frame: checkpoint.frame,
            surface,
            evidence,
        })
    }


    pub fn voxel_count(&self) -> usize {
        self.surface.voxel_count()
    }

    pub fn set_object_pose_session(
        &mut self,
        object_id: &str,
        object_pose_session: Pose,
    ) -> Result<(), ReconstructionFrameError> {
        if !pose_is_finite(object_pose_session) {
            return Err(ReconstructionFrameError::InvalidPose);
        }

        match &mut self.frame {
            ReconstructionFrame::SessionWorld => Err(ReconstructionFrameError::NotObjectLocal),
            ReconstructionFrame::ObjectLocal {
                object_id: existing_id,
                object_pose_session: existing_pose,
            } => {
                if existing_id != object_id {
                    return Err(ReconstructionFrameError::ObjectIdMismatch);
                }
                *existing_pose = object_pose_session;
                Ok(())
            }
        }
    }

    pub fn integrate_observed_frame(
        &mut self,
        camera_pose_session: Pose,
        samples: &[SurfaceSample],
        provenance: &ObservationProvenance,
    ) -> IntegrationReport {
        let camera_pose_frame = match self.camera_pose_in_frame(camera_pose_session) {
            Some(pose) => pose,
            None => {
                return IntegrationReport {
                    rejected_samples: samples.len(),
                    ..IntegrationReport::default()
                };
            }
        };

        let report = self.surface.integrate_frame(camera_pose_frame, samples);

        for sample in samples {
            if sample_weight(*sample, self.surface.config.max_sample_weight).is_none() {
                continue;
            }
            let point_frame = transform_point(camera_pose_frame, sample.position_camera_meters);
            let Some(key) = voxel_key(point_frame, self.surface.config.voxel_size_meters) else {
                continue;
            };
            if !self.surface.voxels.contains_key(&key) {
                continue;
            }

            let evidence = self.evidence.entry(key).or_default();
            insert_bounded_unique(
                &mut evidence.observation_ids,
                &provenance.observation_id,
                MAX_PROVENANCE_IDS_PER_VOXEL,
            );
            insert_bounded_unique(
                &mut evidence.device_ids,
                &provenance.device_id,
                MAX_PROVENANCE_IDS_PER_VOXEL,
            );
            if let Some(timestamp) = provenance.session_timestamp_micros {
                evidence.first_session_timestamp_micros = Some(
                    evidence
                        .first_session_timestamp_micros
                        .map_or(timestamp, |current| current.min(timestamp)),
                );
                evidence.last_session_timestamp_micros = Some(
                    evidence
                        .last_session_timestamp_micros
                        .map_or(timestamp, |current| current.max(timestamp)),
                );
            }
        }

        self.evidence
            .retain(|key, _| self.surface.voxels.contains_key(key));

        report
    }

    pub fn voxel_evidence(&self, key: VoxelKey) -> Option<&VoxelEvidence> {
        self.evidence.get(&key)
    }

    pub fn extract_points(&self, min_confidence: f32) -> Vec<SurfacePoint> {
        self.surface
            .extract_points(min_confidence)
            .into_iter()
            .map(|mut point| {
                point.position_session_meters =
                    self.frame_point_to_session(point.position_session_meters);
                point
            })
            .collect()
    }

    pub fn extract_preview_mesh(&self, min_confidence: f32) -> PreviewMesh {
        let mut mesh = self.surface.extract_preview_mesh(min_confidence);
        for vertex in &mut mesh.vertices_session_meters {
            *vertex = self.frame_point_to_session(*vertex);
        }
        mesh
    }

    pub fn raycast_surface(
        &self,
        origin_session_meters: Vec3,
        direction_session: Vec3,
        min_confidence: f32,
    ) -> Option<RaycastHit> {
        let origin_frame = self.session_point_to_frame(origin_session_meters)?;
        let direction_frame = self.session_direction_to_frame(direction_session)?;
        let mut hit =
            self.surface
                .raycast_surface(origin_frame, direction_frame, min_confidence)?;
        hit.position_session_meters =
            self.frame_point_to_session(hit.position_session_meters);
        hit.distance_meters =
            distance(origin_session_meters, hit.position_session_meters);
        Some(hit)
    }

    fn camera_pose_in_frame(&self, camera_pose_session: Pose) -> Option<Pose> {
        if !pose_is_finite(camera_pose_session) {
            return None;
        }
        match &self.frame {
            ReconstructionFrame::SessionWorld => Some(camera_pose_session),
            ReconstructionFrame::ObjectLocal {
                object_pose_session, ..
            } => Some(compose_pose(
                inverse_pose(*object_pose_session)?,
                camera_pose_session,
            )),
        }
    }

    fn session_point_to_frame(&self, point_session: Vec3) -> Option<Vec3> {
        match &self.frame {
            ReconstructionFrame::SessionWorld => Some(point_session),
            ReconstructionFrame::ObjectLocal {
                object_pose_session, ..
            } => Some(transform_point(inverse_pose(*object_pose_session)?, point_session)),
        }
    }

    fn frame_point_to_session(&self, point_frame: Vec3) -> Vec3 {
        match &self.frame {
            ReconstructionFrame::SessionWorld => point_frame,
            ReconstructionFrame::ObjectLocal {
                object_pose_session, ..
            } => transform_point(*object_pose_session, point_frame),
        }
    }

    fn session_direction_to_frame(&self, direction_session: Vec3) -> Option<Vec3> {
        match &self.frame {
            ReconstructionFrame::SessionWorld => normalize_vec(direction_session),
            ReconstructionFrame::ObjectLocal {
                object_pose_session, ..
            } => {
                let inverse = inverse_pose(*object_pose_session)?;
                normalize_vec(rotate_vec(inverse.orientation, direction_session))
            }
        }
    }
}

fn insert_bounded_unique(values: &mut Vec<String>, value: &str, limit: usize) {
    if values.iter().any(|existing| existing == value) {
        return;
    }
    if values.len() >= limit {
        values.remove(0);
    }
    values.push(value.to_owned());
}

fn compose_pose(a: Pose, b: Pose) -> Pose {
    Pose {
        position_meters: add_vec(
            a.position_meters,
            rotate_vec(a.orientation, b.position_meters),
        ),
        orientation: normalize_quaternion_value(multiply_quaternion(a.orientation, b.orientation)),
    }
}

fn inverse_pose(pose: Pose) -> Option<Pose> {
    let q = normalized_quaternion(pose.orientation)?;
    let inverse_orientation = Quaternion {
        x: -q.x,
        y: -q.y,
        z: -q.z,
        w: q.w,
    };
    Some(Pose {
        position_meters: rotate_vec(
            inverse_orientation,
            scale_vec(pose.position_meters, -1.0),
        ),
        orientation: inverse_orientation,
    })
}

fn multiply_quaternion(a: Quaternion, b: Quaternion) -> Quaternion {
    Quaternion {
        w: a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z,
        x: a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
        y: a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
        z: a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w,
    }
}

fn normalize_quaternion_value(q: Quaternion) -> Quaternion {
    normalized_quaternion(q).unwrap_or(Quaternion {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    })
}

fn pose_is_finite(pose: Pose) -> bool {
    vec_is_finite(pose.position_meters)
        && pose.orientation.x.is_finite()
        && pose.orientation.y.is_finite()
        && pose.orientation.z.is_finite()
        && pose.orientation.w.is_finite()
        && normalized_quaternion(pose.orientation).is_some()
}

fn ray_aabb_distance(origin: Vec3, direction: Vec3, min: Vec3, max: Vec3) -> Option<f64> {
    let mut t_min: f64 = 0.0;
    let mut t_max: f64 = f64::INFINITY;

    for (origin_component, direction_component, min_component, max_component) in [
        (origin.x, direction.x, min.x, max.x),
        (origin.y, direction.y, min.y, max.y),
        (origin.z, direction.z, min.z, max.z),
    ] {
        if direction_component.abs() <= 1e-12 {
            if origin_component < min_component || origin_component > max_component {
                return None;
            }
            continue;
        }

        let inverse = 1.0 / direction_component;
        let mut near = (min_component - origin_component) * inverse;
        let mut far = (max_component - origin_component) * inverse;
        if near > far {
            std::mem::swap(&mut near, &mut far);
        }

        t_min = t_min.max(near);
        t_max = t_max.min(far);
        if t_max < t_min {
            return None;
        }
    }

    (t_max >= 0.0).then_some(t_min.max(0.0))
}

fn normalize_vec(v: Vec3) -> Option<Vec3> {
    if !vec_is_finite(v) {
        return None;
    }
    let magnitude = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
    if !magnitude.is_finite() || magnitude <= 1e-12 {
        return None;
    }
    Some(scale_vec(v, 1.0 / magnitude))
}

fn voxel_confidence(voxel: &VoxelState) -> f32 {
    if voxel.observation_count == 0 {
        return 0.0;
    }
    (voxel.confidence_sum / f64::from(voxel.observation_count)).clamp(0.0, 1.0) as f32
}

fn offset_key(key: VoxelKey, delta: VoxelKey) -> Option<VoxelKey> {
    Some(VoxelKey {
        x: key.x.checked_add(delta.x)?,
        y: key.y.checked_add(delta.y)?,
        z: key.z.checked_add(delta.z)?,
    })
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

fn distance(a: Vec3, b: Vec3) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
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
    fn preview_mesh_culls_faces_between_adjacent_voxels() {
        let mut volume = SparseSurfaceVolume::new(FusionConfig {
            voxel_size_meters: 0.1,
            max_voxels: 10,
            max_sample_weight: 10_000.0,
        })
        .unwrap();

        volume.integrate_frame(
            identity_pose(),
            &[sample(0.01, 1.0, 0.01), sample(0.11, 1.0, 0.01)],
        );

        let mesh = volume.extract_preview_mesh(0.0);
        assert_eq!(mesh.vertices_session_meters.len(), 40);
        assert_eq!(mesh.triangle_indices.len(), 60);
    }

    #[test]
    fn preview_mesh_respects_confidence_threshold() {
        let mut volume = SparseSurfaceVolume::new(FusionConfig {
            voxel_size_meters: 0.1,
            max_voxels: 10,
            max_sample_weight: 10_000.0,
        })
        .unwrap();

        volume.integrate_frame(identity_pose(), &[sample(0.01, 0.3, 0.01)]);

        assert!(volume.extract_preview_mesh(0.5).triangle_indices.is_empty());
        assert_eq!(volume.extract_preview_mesh(0.2).triangle_indices.len(), 36);
    }

    #[test]
    fn raycast_returns_nearest_confident_surface_voxel() {
        let mut volume = SparseSurfaceVolume::new(FusionConfig {
            voxel_size_meters: 0.1,
            max_voxels: 10,
            max_sample_weight: 10_000.0,
        })
        .unwrap();

        volume.integrate_frame(
            identity_pose(),
            &[sample(0.01, 1.0, 0.01), SurfaceSample {
                position_camera_meters: Vec3 { x: 0.01, y: 0.0, z: 2.0 },
                confidence: 1.0,
                depth_uncertainty_meters: 0.01,
            }],
        );

        let hit = volume
            .raycast_surface(
                Vec3 { x: 0.01, y: 0.01, z: 0.0 },
                Vec3 { x: 0.0, y: 0.0, z: 1.0 },
                0.5,
            )
            .unwrap();

        assert!((hit.distance_meters - 1.0).abs() < 1e-9);
        assert_eq!(hit.voxel.z, 10);
    }

    #[test]
    fn raycast_ignores_surface_below_confidence_threshold() {
        let mut volume = SparseSurfaceVolume::new(FusionConfig {
            voxel_size_meters: 0.1,
            max_voxels: 10,
            max_sample_weight: 10_000.0,
        })
        .unwrap();

        volume.integrate_frame(identity_pose(), &[sample(0.01, 0.2, 0.01)]);

        assert!(volume
            .raycast_surface(
                Vec3 { x: 0.01, y: 0.01, z: 0.0 },
                Vec3 { x: 0.0, y: 0.0, z: 1.0 },
                0.5,
            )
            .is_none());
    }

    #[test]
    fn sparse_checkpoint_resume_matches_uninterrupted_fusion() {
        let config = FusionConfig {
            voxel_size_meters: 0.1,
            max_voxels: 10,
            max_sample_weight: 10_000.0,
        };
        let first = sample(0.011, 1.0, 0.01);
        let second = sample(0.019, 1.0, 0.01);

        let mut uninterrupted = SparseSurfaceVolume::new(config.clone()).unwrap();
        uninterrupted.integrate_frame(identity_pose(), &[first]);
        uninterrupted.integrate_frame(identity_pose(), &[second]);

        let mut interrupted = SparseSurfaceVolume::new(config).unwrap();
        interrupted.integrate_frame(identity_pose(), &[first]);
        let checkpoint = interrupted.checkpoint();
        let mut resumed = SparseSurfaceVolume::from_checkpoint(checkpoint).unwrap();
        resumed.integrate_frame(identity_pose(), &[second]);

        assert_eq!(resumed.extract_points(0.0), uninterrupted.extract_points(0.0));
    }

    #[test]
    fn reconstruction_checkpoint_preserves_frame_and_provenance() {
        let mut volume = ReconstructionVolume::new(
            FusionConfig {
                voxel_size_meters: 0.1,
                max_voxels: 10,
                max_sample_weight: 10_000.0,
            },
            ReconstructionFrame::ObjectLocal {
                object_id: "object-a".into(),
                object_pose_session: Pose {
                    position_meters: Vec3 {
                        x: 2.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    ..identity_pose()
                },
            },
        )
        .unwrap();

        volume.integrate_observed_frame(
            Pose {
                position_meters: Vec3 {
                    x: 2.0,
                    y: 0.0,
                    z: 0.0,
                },
                ..identity_pose()
            },
            &[sample(0.01, 0.9, 0.01)],
            &ObservationProvenance {
                observation_id: "frame-a".into(),
                device_id: "phone-a".into(),
                session_timestamp_micros: Some(123),
            },
        );

        let checkpoint = volume.checkpoint();
        let restored = ReconstructionVolume::from_checkpoint(checkpoint).unwrap();

        assert_eq!(restored.frame(), volume.frame());
        assert_eq!(restored.extract_points(0.0), volume.extract_points(0.0));
        assert_eq!(
            restored
                .voxel_evidence(VoxelKey { x: 0, y: 0, z: 10 })
                .unwrap()
                .observation_ids,
            vec!["frame-a"]
        );
    }

    #[test]
    fn reconstruction_restore_rejects_evidence_for_missing_voxel() {
        let checkpoint = ReconstructionCheckpoint {
            frame: ReconstructionFrame::SessionWorld,
            surface: SparseSurfaceCheckpoint {
                config: FusionConfig {
                    voxel_size_meters: 0.1,
                    max_voxels: 10,
                    max_sample_weight: 10_000.0,
                },
                voxels: Vec::new(),
                update_sequence: 0,
            },
            evidence: vec![VoxelEvidenceCheckpoint {
                key: VoxelKey { x: 0, y: 0, z: 0 },
                evidence: VoxelEvidence::default(),
            }],
        };

        assert!(matches!(
            ReconstructionVolume::from_checkpoint(checkpoint),
            Err(ReconstructionRestoreError::EvidenceForMissingVoxel)
        ));
    }

    #[test]
    fn object_local_geometry_can_move_without_reintegrating_surface() {
        let mut volume = ReconstructionVolume::new(
            FusionConfig {
                voxel_size_meters: 0.1,
                max_voxels: 10,
                max_sample_weight: 10_000.0,
            },
            ReconstructionFrame::ObjectLocal {
                object_id: "object-a".into(),
                object_pose_session: Pose {
                    position_meters: Vec3 {
                        x: 2.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    ..identity_pose()
                },
            },
        )
        .unwrap();

        let provenance = ObservationProvenance {
            observation_id: "frame-1".into(),
            device_id: "scanner".into(),
            session_timestamp_micros: Some(100),
        };
        let camera_pose = Pose {
            position_meters: Vec3 {
                x: 2.0,
                y: 0.0,
                z: 0.0,
            },
            ..identity_pose()
        };
        volume.integrate_observed_frame(camera_pose, &[sample(0.0, 1.0, 0.01)], &provenance);

        let before = volume.extract_points(0.0)[0].position_session_meters;
        assert!((before.x - 2.0).abs() < 1e-9);

        volume
            .set_object_pose_session(
                "object-a",
                Pose {
                    position_meters: Vec3 {
                        x: 5.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    ..identity_pose()
                },
            )
            .unwrap();

        let after = volume.extract_points(0.0)[0].position_session_meters;
        assert!((after.x - 5.0).abs() < 1e-9);
        assert_eq!(volume.voxel_count(), 1);
    }

    #[test]
    fn voxel_provenance_tracks_distinct_observations_and_devices() {
        let mut volume = ReconstructionVolume::new(
            FusionConfig {
                voxel_size_meters: 0.1,
                max_voxels: 10,
                max_sample_weight: 10_000.0,
            },
            ReconstructionFrame::SessionWorld,
        )
        .unwrap();

        for (observation_id, device_id, timestamp) in [
            ("frame-a", "phone-a", 100),
            ("frame-b", "phone-b", 200),
        ] {
            volume.integrate_observed_frame(
                identity_pose(),
                &[sample(0.01, 0.9, 0.01)],
                &ObservationProvenance {
                    observation_id: observation_id.into(),
                    device_id: device_id.into(),
                    session_timestamp_micros: Some(timestamp),
                },
            );
        }

        let key = VoxelKey { x: 0, y: 0, z: 10 };
        let evidence = volume.voxel_evidence(key).unwrap();
        assert_eq!(evidence.observation_ids, vec!["frame-a", "frame-b"]);
        assert_eq!(evidence.device_ids, vec!["phone-a", "phone-b"]);
        assert_eq!(evidence.first_session_timestamp_micros, Some(100));
        assert_eq!(evidence.last_session_timestamp_micros, Some(200));
    }

    #[test]
    fn object_local_raycast_returns_session_registered_hit() {
        let mut volume = ReconstructionVolume::new(
            FusionConfig {
                voxel_size_meters: 0.1,
                max_voxels: 10,
                max_sample_weight: 10_000.0,
            },
            ReconstructionFrame::ObjectLocal {
                object_id: "object-a".into(),
                object_pose_session: Pose {
                    position_meters: Vec3 {
                        x: 3.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    ..identity_pose()
                },
            },
        )
        .unwrap();

        volume.integrate_observed_frame(
            Pose {
                position_meters: Vec3 {
                    x: 3.0,
                    y: 0.0,
                    z: 0.0,
                },
                ..identity_pose()
            },
            &[sample(0.01, 1.0, 0.01)],
            &ObservationProvenance {
                observation_id: "frame".into(),
                device_id: "scanner".into(),
                session_timestamp_micros: None,
            },
        );

        let hit = volume
            .raycast_surface(
                Vec3 {
                    x: 3.01,
                    y: 0.01,
                    z: 0.0,
                },
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
                0.5,
            )
            .unwrap();

        assert!((hit.position_session_meters.x - 3.01).abs() < 1e-9);
        assert!((hit.distance_meters - 1.0).abs() < 1e-9);
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
