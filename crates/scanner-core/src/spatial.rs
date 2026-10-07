use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::observation::{Pose, Quaternion, Vec3};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConstraintSource {
    VisualInertial,
    VisualPeer,
    Uwb,
    BluetoothRanging,
    Structural,
    ManualCorrespondence,
    PlatformPose,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Uncertainty {
    pub standard_deviation: f64,
}

impl Uncertainty {
    pub fn bounded(self) -> Self {
        Self {
            standard_deviation: self.standard_deviation.max(1e-6),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SpatialConstraintKind {
    AbsolutePose {
        device_id: String,
        pose_session: Pose,
    },
    RelativePose {
        from_device_id: String,
        to_device_id: String,
        from_to: Pose,
    },
    Range {
        from_device_id: String,
        to_device_id: String,
        distance_meters: f64,
        direction_from_device: Option<Vec3>,
    },
    GravityAlignment {
        device_id: String,
        gravity_device: Vec3,
    },
    StructuralPlane {
        device_id: String,
        surface_id: String,
        normal_device: Vec3,
        offset_meters: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpatialConstraint {
    pub id: String,
    pub epoch: u64,
    pub source: ConstraintSource,
    pub uncertainty: Uncertainty,
    pub kind: SpatialConstraintKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConstraintRejection {
    DuplicateId,
    StaleEpoch { expected: u64, actual: u64 },
    InvalidMeasurement,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintEvaluation {
    pub id: String,
    pub residual: Option<f64>,
    pub robust_weight: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SolveReport {
    pub transforms_added: usize,
    pub unresolved_devices: Vec<String>,
    pub evaluations: Vec<ConstraintEvaluation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OptimizationReport {
    pub iterations: u32,
    pub initial_cost: f64,
    pub final_cost: f64,
    pub max_applied_step_meters: f64,
}

#[derive(Debug, Clone)]
pub struct SessionWorld {
    epoch: u64,
    constraints: Vec<SpatialConstraint>,
    constraint_ids: BTreeSet<String>,
    device_poses: BTreeMap<String, Pose>,
}

impl SessionWorld {
    pub fn new(epoch: u64) -> Self {
        Self {
            epoch,
            constraints: Vec::new(),
            constraint_ids: BTreeSet::new(),
            device_poses: BTreeMap::new(),
        }
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn device_pose(&self, device_id: &str) -> Option<&Pose> {
        self.device_poses.get(device_id)
    }

    pub fn constraints(&self) -> &[SpatialConstraint] {
        &self.constraints
    }

    pub fn add_constraint(
        &mut self,
        constraint: SpatialConstraint,
    ) -> Result<(), ConstraintRejection> {
        if constraint.epoch != self.epoch {
            return Err(ConstraintRejection::StaleEpoch {
                expected: self.epoch,
                actual: constraint.epoch,
            });
        }
        if !self.constraint_ids.insert(constraint.id.clone()) {
            return Err(ConstraintRejection::DuplicateId);
        }
        if !constraint_is_valid(&constraint) {
            self.constraint_ids.remove(&constraint.id);
            return Err(ConstraintRejection::InvalidMeasurement);
        }
        self.constraints.push(constraint);
        Ok(())
    }

    pub fn solve_propagation(&mut self) -> SolveReport {
        let before = self.device_poses.len();

        for constraint in &self.constraints {
            if let SpatialConstraintKind::AbsolutePose {
                device_id,
                pose_session,
            } = &constraint.kind
            {
                self.device_poses
                    .entry(device_id.clone())
                    .or_insert(*pose_session);
            }
        }

        let mut changed = true;
        while changed {
            changed = false;
            for constraint in &self.constraints {
                match &constraint.kind {
                    SpatialConstraintKind::RelativePose {
                        from_device_id,
                        to_device_id,
                        from_to,
                    } => {
                        if self.device_poses.contains_key(to_device_id) {
                            continue;
                        }
                        if let Some(from_pose) = self.device_poses.get(from_device_id).copied() {
                            self.device_poses
                                .insert(to_device_id.clone(), compose_pose(from_pose, *from_to));
                            changed = true;
                        }
                    }
                    SpatialConstraintKind::Range {
                        from_device_id,
                        to_device_id,
                        distance_meters,
                        direction_from_device: Some(direction),
                    } => {
                        if self.device_poses.contains_key(to_device_id) {
                            continue;
                        }
                        if let Some(from_pose) = self.device_poses.get(from_device_id).copied() {
                            let unit = normalize_vec(*direction);
                            if let Some(unit) = unit {
                                let local_offset = scale_vec(unit, *distance_meters);
                                let world_offset =
                                    rotate_vec(from_pose.orientation, local_offset);
                                let pose = Pose {
                                    position_meters: add_vec(
                                        from_pose.position_meters,
                                        world_offset,
                                    ),
                                    orientation: from_pose.orientation,
                                };
                                self.device_poses.insert(to_device_id.clone(), pose);
                                changed = true;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        let evaluations = self
            .constraints
            .iter()
            .map(|constraint| self.evaluate_constraint(constraint))
            .collect::<Vec<_>>();

        let unresolved_devices = referenced_devices(&self.constraints)
            .into_iter()
            .filter(|id| !self.device_poses.contains_key(id))
            .collect();

        SolveReport {
            transforms_added: self.device_poses.len().saturating_sub(before),
            unresolved_devices,
            evaluations,
        }
    }

    pub fn optimize_positions(
        &mut self,
        max_iterations: u32,
        learning_rate: f64,
        max_step_meters: f64,
    ) -> OptimizationReport {
        self.solve_propagation();

        let learning_rate = learning_rate.clamp(0.0, 1.0);
        let max_step_meters = max_step_meters.max(0.0);
        let fixed_devices = self
            .constraints
            .iter()
            .filter_map(|constraint| match &constraint.kind {
                SpatialConstraintKind::AbsolutePose { device_id, .. } => Some(device_id.clone()),
                _ => None,
            })
            .collect::<BTreeSet<_>>();

        let initial_cost = self.position_cost();
        let mut iterations = 0;
        let mut max_applied_step_meters: f64 = 0.0;

        for _ in 0..max_iterations {
            let mut corrections = BTreeMap::<String, Vec3>::new();
            let mut weights = BTreeMap::<String, f64>::new();

            for constraint in &self.constraints {
                let sigma = constraint.uncertainty.bounded().standard_deviation;
                match &constraint.kind {
                    SpatialConstraintKind::RelativePose {
                        from_device_id,
                        to_device_id,
                        from_to,
                    } => {
                        let (Some(from), Some(to)) = (
                            self.device_poses.get(from_device_id).copied(),
                            self.device_poses.get(to_device_id).copied(),
                        ) else {
                            continue;
                        };

                        let predicted_to = add_vec(
                            from.position_meters,
                            rotate_vec(from.orientation, from_to.position_meters),
                        );
                        let residual = sub_vec(to.position_meters, predicted_to);
                        let magnitude = magnitude(residual);
                        let weight =
                            robust_weight(magnitude, sigma) / (sigma * sigma).max(1e-12);

                        if !fixed_devices.contains(from_device_id) {
                            accumulate_correction(
                                &mut corrections,
                                &mut weights,
                                from_device_id,
                                residual,
                                weight,
                            );
                        }
                        if !fixed_devices.contains(to_device_id) {
                            accumulate_correction(
                                &mut corrections,
                                &mut weights,
                                to_device_id,
                                scale_vec(residual, -1.0),
                                weight,
                            );
                        }
                    }
                    SpatialConstraintKind::Range {
                        from_device_id,
                        to_device_id,
                        distance_meters,
                        ..
                    } => {
                        let (Some(from), Some(to)) = (
                            self.device_poses.get(from_device_id).copied(),
                            self.device_poses.get(to_device_id).copied(),
                        ) else {
                            continue;
                        };

                        let delta = sub_vec(to.position_meters, from.position_meters);
                        let current_distance = magnitude(delta);
                        if current_distance <= 1e-12 || !current_distance.is_finite() {
                            continue;
                        }

                        let residual = current_distance - *distance_meters;
                        let unit = scale_vec(delta, 1.0 / current_distance);
                        let correction = scale_vec(unit, residual);
                        let weight =
                            robust_weight(residual, sigma) / (sigma * sigma).max(1e-12);

                        if !fixed_devices.contains(from_device_id) {
                            accumulate_correction(
                                &mut corrections,
                                &mut weights,
                                from_device_id,
                                correction,
                                weight,
                            );
                        }
                        if !fixed_devices.contains(to_device_id) {
                            accumulate_correction(
                                &mut corrections,
                                &mut weights,
                                to_device_id,
                                scale_vec(correction, -1.0),
                                weight,
                            );
                        }
                    }
                    _ => {}
                }
            }

            let mut largest_step: f64 = 0.0;
            for (device_id, correction_sum) in corrections {
                let total_weight = weights.get(&device_id).copied().unwrap_or(0.0);
                if total_weight <= 0.0 {
                    continue;
                }

                let mut step = scale_vec(correction_sum, learning_rate / total_weight);
                let step_length = magnitude(step);
                if step_length > max_step_meters && step_length > 1e-12 {
                    step = scale_vec(step, max_step_meters / step_length);
                }

                let applied = magnitude(step);
                largest_step = largest_step.max(applied);
                max_applied_step_meters = max_applied_step_meters.max(applied);

                if let Some(pose) = self.device_poses.get_mut(&device_id) {
                    pose.position_meters = add_vec(pose.position_meters, step);
                }
            }

            iterations += 1;
            if largest_step <= 1e-8 {
                break;
            }
        }

        OptimizationReport {
            iterations,
            initial_cost,
            final_cost: self.position_cost(),
            max_applied_step_meters,
        }
    }

    fn position_cost(&self) -> f64 {
        self.constraints
            .iter()
            .filter_map(|constraint| {
                let sigma = constraint.uncertainty.bounded().standard_deviation;
                match &constraint.kind {
                    SpatialConstraintKind::RelativePose {
                        from_device_id,
                        to_device_id,
                        from_to,
                    } => {
                        let from = self.device_poses.get(from_device_id)?;
                        let to = self.device_poses.get(to_device_id)?;
                        let predicted_to = add_vec(
                            from.position_meters,
                            rotate_vec(from.orientation, from_to.position_meters),
                        );
                        let residual = magnitude(sub_vec(to.position_meters, predicted_to));
                        Some(robust_cost(residual, sigma))
                    }
                    SpatialConstraintKind::Range {
                        from_device_id,
                        to_device_id,
                        distance_meters,
                        ..
                    } => {
                        let from = self.device_poses.get(from_device_id)?;
                        let to = self.device_poses.get(to_device_id)?;
                        let residual =
                            distance(from.position_meters, to.position_meters) - *distance_meters;
                        Some(robust_cost(residual, sigma))
                    }
                    _ => None,
                }
            })
            .sum()
    }

    fn evaluate_constraint(&self, constraint: &SpatialConstraint) -> ConstraintEvaluation {
        match &constraint.kind {
            SpatialConstraintKind::Range {
                from_device_id,
                to_device_id,
                distance_meters,
                ..
            } => {
                let residual = match (
                    self.device_poses.get(from_device_id),
                    self.device_poses.get(to_device_id),
                ) {
                    (Some(from), Some(to)) => Some(
                        distance(from.position_meters, to.position_meters) - *distance_meters,
                    ),
                    _ => None,
                };
                ConstraintEvaluation {
                    id: constraint.id.clone(),
                    robust_weight: residual.map_or(0.0, |r| {
                        robust_weight(r, constraint.uncertainty.bounded().standard_deviation)
                    }),
                    residual,
                }
            }
            SpatialConstraintKind::RelativePose {
                from_device_id,
                to_device_id,
                from_to,
            } => {
                let residual = match (
                    self.device_poses.get(from_device_id),
                    self.device_poses.get(to_device_id),
                ) {
                    (Some(from), Some(to)) => {
                        let predicted = compose_pose(*from, *from_to);
                        Some(distance(predicted.position_meters, to.position_meters))
                    }
                    _ => None,
                };
                ConstraintEvaluation {
                    id: constraint.id.clone(),
                    robust_weight: residual.map_or(0.0, |r| {
                        robust_weight(r, constraint.uncertainty.bounded().standard_deviation)
                    }),
                    residual,
                }
            }
            SpatialConstraintKind::AbsolutePose {
                device_id,
                pose_session,
            } => {
                let residual = self.device_poses.get(device_id).map(|actual| {
                    distance(actual.position_meters, pose_session.position_meters)
                });
                ConstraintEvaluation {
                    id: constraint.id.clone(),
                    robust_weight: residual.map_or(0.0, |r| {
                        robust_weight(r, constraint.uncertainty.bounded().standard_deviation)
                    }),
                    residual,
                }
            }
            _ => ConstraintEvaluation {
                id: constraint.id.clone(),
                residual: None,
                robust_weight: 1.0,
            },
        }
    }
}

fn constraint_is_valid(constraint: &SpatialConstraint) -> bool {
    if !constraint.uncertainty.standard_deviation.is_finite()
        || constraint.uncertainty.standard_deviation <= 0.0
    {
        return false;
    }
    match &constraint.kind {
        SpatialConstraintKind::Range {
            distance_meters,
            direction_from_device,
            ..
        } => {
            distance_meters.is_finite()
                && *distance_meters >= 0.0
                && direction_from_device
                    .map(|v| vec_is_finite(v) && magnitude(v) > 1e-9)
                    .unwrap_or(true)
        }
        SpatialConstraintKind::AbsolutePose { pose_session, .. }
        | SpatialConstraintKind::RelativePose {
            from_to: pose_session,
            ..
        } => pose_is_finite(*pose_session),
        SpatialConstraintKind::GravityAlignment { gravity_device, .. } => {
            vec_is_finite(*gravity_device) && magnitude(*gravity_device) > 1e-9
        }
        SpatialConstraintKind::StructuralPlane {
            normal_device,
            offset_meters,
            ..
        } => {
            vec_is_finite(*normal_device)
                && magnitude(*normal_device) > 1e-9
                && offset_meters.is_finite()
        }
    }
}

fn referenced_devices(constraints: &[SpatialConstraint]) -> BTreeSet<String> {
    let mut devices = BTreeSet::new();
    for constraint in constraints {
        match &constraint.kind {
            SpatialConstraintKind::AbsolutePose { device_id, .. }
            | SpatialConstraintKind::GravityAlignment { device_id, .. }
            | SpatialConstraintKind::StructuralPlane { device_id, .. } => {
                devices.insert(device_id.clone());
            }
            SpatialConstraintKind::RelativePose {
                from_device_id,
                to_device_id,
                ..
            }
            | SpatialConstraintKind::Range {
                from_device_id,
                to_device_id,
                ..
            } => {
                devices.insert(from_device_id.clone());
                devices.insert(to_device_id.clone());
            }
        }
    }
    devices
}

fn compose_pose(a: Pose, b: Pose) -> Pose {
    Pose {
        position_meters: add_vec(
            a.position_meters,
            rotate_vec(a.orientation, b.position_meters),
        ),
        orientation: normalize_quaternion(multiply_quaternion(a.orientation, b.orientation)),
    }
}

fn multiply_quaternion(a: Quaternion, b: Quaternion) -> Quaternion {
    Quaternion {
        w: a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z,
        x: a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
        y: a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
        z: a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w,
    }
}

fn normalize_quaternion(q: Quaternion) -> Quaternion {
    let norm = (q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w).sqrt();
    if norm <= 1e-12 || !norm.is_finite() {
        return Quaternion {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        };
    }
    Quaternion {
        x: q.x / norm,
        y: q.y / norm,
        z: q.z / norm,
        w: q.w / norm,
    }
}

fn rotate_vec(q: Quaternion, v: Vec3) -> Vec3 {
    let q = normalize_quaternion(q);
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

fn robust_weight(residual: f64, sigma: f64) -> f64 {
    let normalized = residual.abs() / (3.0 * sigma.max(1e-6));
    1.0 / (1.0 + normalized * normalized)
}

fn vec_is_finite(v: Vec3) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.z.is_finite()
}

fn pose_is_finite(pose: Pose) -> bool {
    vec_is_finite(pose.position_meters)
        && pose.orientation.x.is_finite()
        && pose.orientation.y.is_finite()
        && pose.orientation.z.is_finite()
        && pose.orientation.w.is_finite()
}

fn magnitude(v: Vec3) -> f64 {
    dot(v, v).sqrt()
}

fn normalize_vec(v: Vec3) -> Option<Vec3> {
    let m = magnitude(v);
    if m <= 1e-12 || !m.is_finite() {
        return None;
    }
    Some(scale_vec(v, 1.0 / m))
}

fn add_vec(a: Vec3, b: Vec3) -> Vec3 {
    Vec3 {
        x: a.x + b.x,
        y: a.y + b.y,
        z: a.z + b.z,
    }
}

fn sub_vec(a: Vec3, b: Vec3) -> Vec3 {
    Vec3 {
        x: a.x - b.x,
        y: a.y - b.y,
        z: a.z - b.z,
    }
}

fn accumulate_correction(
    corrections: &mut BTreeMap<String, Vec3>,
    weights: &mut BTreeMap<String, f64>,
    device_id: &str,
    correction: Vec3,
    weight: f64,
) {
    if !weight.is_finite() || weight <= 0.0 || !vec_is_finite(correction) {
        return;
    }

    let entry = corrections.entry(device_id.to_owned()).or_insert(Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    });
    *entry = add_vec(*entry, scale_vec(correction, weight));
    *weights.entry(device_id.to_owned()).or_insert(0.0) += weight;
}

fn robust_cost(residual: f64, sigma: f64) -> f64 {
    let scale = 3.0 * sigma.max(1e-6);
    let normalized = residual / scale;
    (1.0 + normalized * normalized).ln()
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

fn distance(a: Vec3, b: Vec3) -> f64 {
    magnitude(Vec3 {
        x: a.x - b.x,
        y: a.y - b.y,
        z: a.z - b.z,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity_pose(x: f64, y: f64, z: f64) -> Pose {
        Pose {
            position_meters: Vec3 { x, y, z },
            orientation: Quaternion {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        }
    }

    fn constraint(id: &str, kind: SpatialConstraintKind) -> SpatialConstraint {
        SpatialConstraint {
            id: id.into(),
            epoch: 3,
            source: ConstraintSource::Uwb,
            uncertainty: Uncertainty {
                standard_deviation: 0.02,
            },
            kind,
        }
    }

    #[test]
    fn relative_pose_propagates_from_known_session_pose() {
        let mut world = SessionWorld::new(3);
        world
            .add_constraint(constraint(
                "root",
                SpatialConstraintKind::AbsolutePose {
                    device_id: "a".into(),
                    pose_session: identity_pose(1.0, 0.0, 0.0),
                },
            ))
            .unwrap();
        world
            .add_constraint(constraint(
                "a-b",
                SpatialConstraintKind::RelativePose {
                    from_device_id: "a".into(),
                    to_device_id: "b".into(),
                    from_to: identity_pose(2.0, 0.0, 0.0),
                },
            ))
            .unwrap();

        let report = world.solve_propagation();
        assert_eq!(report.transforms_added, 2);
        assert_eq!(
            world.device_pose("b").unwrap().position_meters,
            Vec3 {
                x: 3.0,
                y: 0.0,
                z: 0.0
            }
        );
    }

    #[test]
    fn directed_range_can_seed_peer_position_without_becoming_world_authority() {
        let mut world = SessionWorld::new(3);
        world
            .add_constraint(constraint(
                "root",
                SpatialConstraintKind::AbsolutePose {
                    device_id: "anchor".into(),
                    pose_session: identity_pose(0.0, 0.0, 0.0),
                },
            ))
            .unwrap();
        world
            .add_constraint(constraint(
                "range",
                SpatialConstraintKind::Range {
                    from_device_id: "anchor".into(),
                    to_device_id: "scanner".into(),
                    distance_meters: 2.5,
                    direction_from_device: Some(Vec3 {
                        x: 1.0,
                        y: 0.0,
                        z: 0.0,
                    }),
                },
            ))
            .unwrap();

        world.solve_propagation();
        assert_eq!(
            world.device_pose("scanner").unwrap().position_meters,
            Vec3 {
                x: 2.5,
                y: 0.0,
                z: 0.0
            }
        );
    }

    #[test]
    fn undirected_range_does_not_invent_an_unobservable_position() {
        let mut world = SessionWorld::new(3);
        world
            .add_constraint(constraint(
                "root",
                SpatialConstraintKind::AbsolutePose {
                    device_id: "a".into(),
                    pose_session: identity_pose(0.0, 0.0, 0.0),
                },
            ))
            .unwrap();
        world
            .add_constraint(constraint(
                "range",
                SpatialConstraintKind::Range {
                    from_device_id: "a".into(),
                    to_device_id: "b".into(),
                    distance_meters: 1.0,
                    direction_from_device: None,
                },
            ))
            .unwrap();

        let report = world.solve_propagation();
        assert!(world.device_pose("b").is_none());
        assert_eq!(report.unresolved_devices, vec!["b"]);
    }

    #[test]
    fn contradictory_range_is_downweighted_instead_of_teleporting_devices() {
        let mut world = SessionWorld::new(3);
        world
            .add_constraint(constraint(
                "a",
                SpatialConstraintKind::AbsolutePose {
                    device_id: "a".into(),
                    pose_session: identity_pose(0.0, 0.0, 0.0),
                },
            ))
            .unwrap();
        world
            .add_constraint(constraint(
                "b",
                SpatialConstraintKind::AbsolutePose {
                    device_id: "b".into(),
                    pose_session: identity_pose(2.0, 0.0, 0.0),
                },
            ))
            .unwrap();
        world
            .add_constraint(constraint(
                "bad-range",
                SpatialConstraintKind::Range {
                    from_device_id: "a".into(),
                    to_device_id: "b".into(),
                    distance_meters: 8.0,
                    direction_from_device: None,
                },
            ))
            .unwrap();

        let report = world.solve_propagation();
        let evaluation = report
            .evaluations
            .iter()
            .find(|e| e.id == "bad-range")
            .unwrap();

        assert!(evaluation.robust_weight < 0.01);
        assert_eq!(
            world.device_pose("b").unwrap().position_meters,
            Vec3 {
                x: 2.0,
                y: 0.0,
                z: 0.0
            }
        );
    }

    #[test]
    fn joint_refinement_reduces_conflicting_position_cost_without_moving_anchor() {
        let mut world = SessionWorld::new(3);
        world
            .add_constraint(SpatialConstraint {
                id: "root".into(),
                epoch: 3,
                source: ConstraintSource::PlatformPose,
                uncertainty: Uncertainty {
                    standard_deviation: 0.01,
                },
                kind: SpatialConstraintKind::AbsolutePose {
                    device_id: "a".into(),
                    pose_session: identity_pose(0.0, 0.0, 0.0),
                },
            })
            .unwrap();
        world
            .add_constraint(SpatialConstraint {
                id: "seed-range".into(),
                epoch: 3,
                source: ConstraintSource::Uwb,
                uncertainty: Uncertainty {
                    standard_deviation: 0.5,
                },
                kind: SpatialConstraintKind::Range {
                    from_device_id: "a".into(),
                    to_device_id: "b".into(),
                    distance_meters: 5.0,
                    direction_from_device: Some(Vec3 {
                        x: 1.0,
                        y: 0.0,
                        z: 0.0,
                    }),
                },
            })
            .unwrap();
        world
            .add_constraint(SpatialConstraint {
                id: "visual".into(),
                epoch: 3,
                source: ConstraintSource::VisualPeer,
                uncertainty: Uncertainty {
                    standard_deviation: 0.05,
                },
                kind: SpatialConstraintKind::RelativePose {
                    from_device_id: "a".into(),
                    to_device_id: "b".into(),
                    from_to: identity_pose(2.0, 0.0, 0.0),
                },
            })
            .unwrap();

        world.solve_propagation();
        assert_eq!(world.device_pose("b").unwrap().position_meters.x, 5.0);

        let before_anchor = *world.device_pose("a").unwrap();
        let report = world.optimize_positions(80, 0.5, 0.1);

        assert!(report.final_cost < report.initial_cost);
        assert!(world.device_pose("b").unwrap().position_meters.x < 3.0);
        assert_eq!(*world.device_pose("a").unwrap(), before_anchor);
        assert!(report.max_applied_step_meters <= 0.1000001);
    }

    #[test]
    fn bad_range_is_robustly_downweighted_during_joint_refinement() {
        let mut world = SessionWorld::new(3);
        world
            .add_constraint(SpatialConstraint {
                id: "root".into(),
                epoch: 3,
                source: ConstraintSource::PlatformPose,
                uncertainty: Uncertainty {
                    standard_deviation: 0.01,
                },
                kind: SpatialConstraintKind::AbsolutePose {
                    device_id: "a".into(),
                    pose_session: identity_pose(0.0, 0.0, 0.0),
                },
            })
            .unwrap();
        world
            .add_constraint(SpatialConstraint {
                id: "visual".into(),
                epoch: 3,
                source: ConstraintSource::VisualPeer,
                uncertainty: Uncertainty {
                    standard_deviation: 0.03,
                },
                kind: SpatialConstraintKind::RelativePose {
                    from_device_id: "a".into(),
                    to_device_id: "b".into(),
                    from_to: identity_pose(2.0, 0.0, 0.0),
                },
            })
            .unwrap();
        world
            .add_constraint(SpatialConstraint {
                id: "bad-range".into(),
                epoch: 3,
                source: ConstraintSource::BluetoothRanging,
                uncertainty: Uncertainty {
                    standard_deviation: 0.2,
                },
                kind: SpatialConstraintKind::Range {
                    from_device_id: "a".into(),
                    to_device_id: "b".into(),
                    distance_meters: 20.0,
                    direction_from_device: None,
                },
            })
            .unwrap();

        world.solve_propagation();
        let report = world.optimize_positions(50, 0.4, 0.05);
        let b = world.device_pose("b").unwrap().position_meters.x;

        assert!(report.final_cost <= report.initial_cost);
        assert!((b - 2.0).abs() < 0.25);
    }

    #[test]
    fn scanner_handoff_does_not_redefine_session_world_epoch_or_existing_pose() {
        let mut world = SessionWorld::new(9);
        world
            .add_constraint(SpatialConstraint {
                id: "scanner-a".into(),
                epoch: 9,
                source: ConstraintSource::PlatformPose,
                uncertainty: Uncertainty {
                    standard_deviation: 0.02,
                },
                kind: SpatialConstraintKind::AbsolutePose {
                    device_id: "scanner-a".into(),
                    pose_session: identity_pose(1.0, 0.0, 0.0),
                },
            })
            .unwrap();
        world.solve_propagation();
        let original = *world.device_pose("scanner-a").unwrap();

        world
            .add_constraint(SpatialConstraint {
                id: "scanner-b".into(),
                epoch: 9,
                source: ConstraintSource::VisualPeer,
                uncertainty: Uncertainty {
                    standard_deviation: 0.05,
                },
                kind: SpatialConstraintKind::RelativePose {
                    from_device_id: "scanner-a".into(),
                    to_device_id: "scanner-b".into(),
                    from_to: identity_pose(0.5, 0.0, 0.0),
                },
            })
            .unwrap();
        world.solve_propagation();

        assert_eq!(world.epoch(), 9);
        assert_eq!(*world.device_pose("scanner-a").unwrap(), original);
        assert_eq!(
            world.device_pose("scanner-b").unwrap().position_meters.x,
            1.5
        );
    }

    #[test]
    fn rejects_duplicate_or_wrong_epoch_constraints() {
        let mut world = SessionWorld::new(3);
        let c = constraint(
            "same",
            SpatialConstraintKind::AbsolutePose {
                device_id: "a".into(),
                pose_session: identity_pose(0.0, 0.0, 0.0),
            },
        );
        world.add_constraint(c.clone()).unwrap();
        assert_eq!(
            world.add_constraint(c),
            Err(ConstraintRejection::DuplicateId)
        );

        let mut stale = constraint(
            "stale",
            SpatialConstraintKind::AbsolutePose {
                device_id: "a".into(),
                pose_session: identity_pose(0.0, 0.0, 0.0),
            },
        );
        stale.epoch = 2;
        assert_eq!(
            world.add_constraint(stale),
            Err(ConstraintRejection::StaleEpoch {
                expected: 3,
                actual: 2
            })
        );
    }
}
