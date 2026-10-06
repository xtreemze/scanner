use serde::{Deserialize, Serialize};

use crate::observation::Vec3;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfidenceField {
    pub geometry: f32,
    pub pose: f32,
    pub coverage: f32,
    pub view_angle_diversity: f32,
    pub texture: f32,
    pub material: f32,
    pub structural: f32,
    pub peer_geometry: f32,
}

impl ConfidenceField {
    pub fn clamped(self) -> Self {
        fn clamp(v: f32) -> f32 {
            v.clamp(0.0, 1.0)
        }
        Self {
            geometry: clamp(self.geometry),
            pose: clamp(self.pose),
            coverage: clamp(self.coverage),
            view_angle_diversity: clamp(self.view_angle_diversity),
            texture: clamp(self.texture),
            material: clamp(self.material),
            structural: clamp(self.structural),
            peer_geometry: clamp(self.peer_geometry),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum MeasurementAction {
    MoveScanner { direction: Vec3, meters: f64 },
    ChangeDistance { meters: f64 },
    ReacquireStructure { surface_id: String },
    MoveAnchor { peer_id: String, direction: Vec3, meters: f64 },
    HoldStill { milliseconds: u32 },
    ConfirmSurface { surface_id: String },
    LockSurface { surface_id: String },
    RescanRegion { region_id: String },
    IlluminateRegion { region_id: String, peer_id: Option<String> },
    CaptureEnvironment,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuidanceCandidate {
    pub action: MeasurementAction,
    pub expected_uncertainty_reduction: f32,
    pub user_effort: f32,
    pub acquisition_seconds: f32,
}

impl GuidanceCandidate {
    pub fn score(&self) -> f32 {
        let benefit = self.expected_uncertainty_reduction.max(0.0);
        let cost = 1.0 + self.user_effort.max(0.0) + self.acquisition_seconds.max(0.0) * 0.15;
        benefit / cost
    }
}

pub fn choose_next_action(candidates: &[GuidanceCandidate]) -> Option<&GuidanceCandidate> {
    candidates.iter().max_by(|a, b| {
        a.score()
            .total_cmp(&b.score())
            .then_with(|| b.user_effort.total_cmp(&a.user_effort))
    })
}

pub fn recommend_for_confidence(confidence: ConfidenceField) -> MeasurementAction {
    let c = confidence.clamped();

    if c.pose < 0.45 || c.structural < 0.45 {
        return MeasurementAction::ReacquireStructure {
            surface_id: "nearest-trusted-structure".into(),
        };
    }
    if c.peer_geometry < 0.4 {
        return MeasurementAction::MoveAnchor {
            peer_id: "weakest-anchor".into(),
            direction: Vec3 { x: 1.0, y: 0.0, z: 0.0 },
            meters: 1.0,
        };
    }
    if c.geometry < 0.55 || c.view_angle_diversity < 0.5 {
        return MeasurementAction::MoveScanner {
            direction: Vec3 { x: 0.0, y: 1.0, z: 0.0 },
            meters: 0.35,
        };
    }
    if c.coverage < 0.65 {
        return MeasurementAction::RescanRegion {
            region_id: "lowest-coverage-region".into(),
        };
    }
    if c.material < 0.55 {
        return MeasurementAction::IlluminateRegion {
            region_id: "lowest-material-confidence-region".into(),
            peer_id: None,
        };
    }
    if c.texture < 0.65 {
        return MeasurementAction::ChangeDistance { meters: -0.25 };
    }

    MeasurementAction::CaptureEnvironment
}

#[cfg(test)]
mod tests {
    use super::*;

    fn high() -> ConfidenceField {
        ConfidenceField {
            geometry: 0.9,
            pose: 0.9,
            coverage: 0.9,
            view_angle_diversity: 0.9,
            texture: 0.9,
            material: 0.9,
            structural: 0.9,
            peer_geometry: 0.9,
        }
    }

    #[test]
    fn weak_pose_preempts_collecting_more_object_detail() {
        let mut c = high();
        c.pose = 0.2;
        assert!(matches!(
            recommend_for_confidence(c),
            MeasurementAction::ReacquireStructure { .. }
        ));
    }

    #[test]
    fn weak_anchor_geometry_is_addressed_before_surface_detail() {
        let mut c = high();
        c.peer_geometry = 0.2;
        c.texture = 0.1;
        assert!(matches!(
            recommend_for_confidence(c),
            MeasurementAction::MoveAnchor { .. }
        ));
    }

    #[test]
    fn candidate_ranking_balances_information_gain_against_effort() {
        let low_effort = GuidanceCandidate {
            action: MeasurementAction::HoldStill { milliseconds: 500 },
            expected_uncertainty_reduction: 0.3,
            user_effort: 0.1,
            acquisition_seconds: 0.5,
        };
        let high_effort = GuidanceCandidate {
            action: MeasurementAction::MoveScanner {
                direction: Vec3 { x: 1.0, y: 0.0, z: 0.0 },
                meters: 2.0,
            },
            expected_uncertainty_reduction: 0.4,
            user_effort: 2.0,
            acquisition_seconds: 4.0,
        };
        let candidates = vec![high_effort, low_effort.clone()];
        assert_eq!(choose_next_action(&candidates), Some(&low_effort));
    }
}
