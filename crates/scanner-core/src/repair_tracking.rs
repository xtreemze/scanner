use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{observation::Vec3, repair::RepairRegion};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairConfidence {
    pub geometry: f32,
    pub texture: f32,
    pub material: f32,
}

impl RepairConfidence {
    pub fn clamped(self) -> Self {
        Self {
            geometry: self.geometry.clamp(0.0, 1.0),
            texture: self.texture.clamp(0.0, 1.0),
            material: self.material.clamp(0.0, 1.0),
        }
    }

    fn is_valid(self) -> bool {
        [self.geometry, self.texture, self.material]
            .into_iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(&value))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairObservation {
    pub observation_id: String,
    pub position_session_meters: Vec3,
    pub evidence: RepairConfidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairStatus {
    pub confidence: RepairConfidence,
    pub target_confidence: f32,
    pub progress: f32,
    pub complete: bool,
    pub accepted_observation_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairTrackerError {
    InvalidTargetConfidence,
    InvalidMinimumInformationGain,
    InvalidInitialConfidence,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RepairObservationDecision {
    Accepted { information_gain: f32 },
    OutsideRegion,
    DuplicateObservation,
    InvalidEvidence,
    InsufficientInformationGain { information_gain: f32 },
    AlreadyComplete,
}

#[derive(Debug, Clone)]
pub struct RepairTracker {
    region: RepairRegion,
    confidence: RepairConfidence,
    target_confidence: f32,
    min_information_gain: f32,
    accepted_observation_ids: BTreeSet<String>,
}

impl RepairTracker {
    pub fn new(
        region: RepairRegion,
        initial_confidence: RepairConfidence,
        target_confidence: f32,
        min_information_gain: f32,
    ) -> Result<Self, RepairTrackerError> {
        if !initial_confidence.is_valid() {
            return Err(RepairTrackerError::InvalidInitialConfidence);
        }
        if !target_confidence.is_finite() || !(0.0..=1.0).contains(&target_confidence) {
            return Err(RepairTrackerError::InvalidTargetConfidence);
        }
        if !min_information_gain.is_finite()
            || !(0.0..=1.0).contains(&min_information_gain)
        {
            return Err(RepairTrackerError::InvalidMinimumInformationGain);
        }

        Ok(Self {
            region,
            confidence: initial_confidence.clamped(),
            target_confidence,
            min_information_gain,
            accepted_observation_ids: BTreeSet::new(),
        })
    }

    pub fn region(&self) -> &RepairRegion {
        &self.region
    }

    pub fn status(&self) -> RepairStatus {
        let requested = [
            (self.region.needs.geometry, self.confidence.geometry),
            (self.region.needs.texture, self.confidence.texture),
            (self.region.needs.material, self.confidence.material),
        ];

        let mut requested_count = 0usize;
        let mut progress_sum = 0.0f32;
        let mut complete = true;

        for (needed, confidence) in requested {
            if !needed {
                continue;
            }
            requested_count += 1;
            progress_sum += if self.target_confidence <= f32::EPSILON {
                1.0
            } else {
                (confidence / self.target_confidence).clamp(0.0, 1.0)
            };
            complete &= confidence >= self.target_confidence;
        }

        if requested_count == 0 {
            complete = true;
        }

        RepairStatus {
            confidence: self.confidence,
            target_confidence: self.target_confidence,
            progress: if requested_count == 0 {
                1.0
            } else {
                progress_sum / requested_count as f32
            },
            complete,
            accepted_observation_count: self.accepted_observation_ids.len(),
        }
    }

    pub fn consider_observation(
        &mut self,
        observation: &RepairObservation,
    ) -> RepairObservationDecision {
        if self.status().complete {
            return RepairObservationDecision::AlreadyComplete;
        }
        if self
            .accepted_observation_ids
            .contains(&observation.observation_id)
        {
            return RepairObservationDecision::DuplicateObservation;
        }
        if !self.region.contains_session_point(observation.position_session_meters) {
            return RepairObservationDecision::OutsideRegion;
        }
        if !observation.evidence.is_valid() {
            return RepairObservationDecision::InvalidEvidence;
        }

        let candidate = RepairConfidence {
            geometry: if self.region.needs.geometry {
                combine_confidence(self.confidence.geometry, observation.evidence.geometry)
            } else {
                self.confidence.geometry
            },
            texture: if self.region.needs.texture {
                combine_confidence(self.confidence.texture, observation.evidence.texture)
            } else {
                self.confidence.texture
            },
            material: if self.region.needs.material {
                combine_confidence(self.confidence.material, observation.evidence.material)
            } else {
                self.confidence.material
            },
        };

        let information_gain =
            requested_information_gain(self.region.needs, self.confidence, candidate);

        if information_gain + f32::EPSILON < self.min_information_gain {
            return RepairObservationDecision::InsufficientInformationGain { information_gain };
        }

        self.confidence = candidate;
        self.accepted_observation_ids
            .insert(observation.observation_id.clone());

        RepairObservationDecision::Accepted { information_gain }
    }
}

fn combine_confidence(current: f32, evidence: f32) -> f32 {
    (1.0 - (1.0 - current) * (1.0 - evidence)).clamp(0.0, 1.0)
}

fn requested_information_gain(
    needs: crate::repair::RepairNeeds,
    before: RepairConfidence,
    after: RepairConfidence,
) -> f32 {
    let mut gain = 0.0;
    let mut count = 0.0;

    if needs.geometry {
        gain += (after.geometry - before.geometry).max(0.0);
        count += 1.0;
    }
    if needs.texture {
        gain += (after.texture - before.texture).max(0.0);
        count += 1.0;
    }
    if needs.material {
        gain += (after.material - before.material).max(0.0);
        count += 1.0;
    }

    if count == 0.0 { 0.0 } else { gain / count }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{repair::RepairNeeds, reconstruction::VoxelKey};

    fn region(needs: RepairNeeds) -> RepairRegion {
        RepairRegion {
            id: "repair".into(),
            center_session_meters: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
            radius_meters: 0.5,
            source_voxel: VoxelKey { x: 0, y: 0, z: 10 },
            source_confidence: 0.4,
            needs,
        }
    }

    #[test]
    fn accepts_only_observations_that_add_requested_information() {
        let mut tracker = RepairTracker::new(
            region(RepairNeeds {
                geometry: true,
                texture: false,
                material: false,
            }),
            RepairConfidence {
                geometry: 0.2,
                texture: 0.8,
                material: 0.8,
            },
            0.8,
            0.05,
        )
        .unwrap();

        let accepted = tracker.consider_observation(&RepairObservation {
            observation_id: "good".into(),
            position_session_meters: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 1.1,
            },
            evidence: RepairConfidence {
                geometry: 0.5,
                texture: 0.0,
                material: 0.0,
            },
        });

        assert!(matches!(
            accepted,
            RepairObservationDecision::Accepted { information_gain } if information_gain > 0.3
        ));

        let weak = tracker.consider_observation(&RepairObservation {
            observation_id: "weak".into(),
            position_session_meters: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 1.1,
            },
            evidence: RepairConfidence {
                geometry: 0.01,
                texture: 1.0,
                material: 1.0,
            },
        });

        assert!(matches!(
            weak,
            RepairObservationDecision::InsufficientInformationGain { .. }
        ));
    }

    #[test]
    fn rejects_outside_and_duplicate_observations() {
        let mut tracker = RepairTracker::new(
            region(RepairNeeds {
                geometry: true,
                texture: true,
                material: false,
            }),
            RepairConfidence {
                geometry: 0.1,
                texture: 0.1,
                material: 0.0,
            },
            0.9,
            0.01,
        )
        .unwrap();

        let outside = tracker.consider_observation(&RepairObservation {
            observation_id: "outside".into(),
            position_session_meters: Vec3 {
                x: 10.0,
                y: 0.0,
                z: 1.0,
            },
            evidence: RepairConfidence {
                geometry: 0.8,
                texture: 0.8,
                material: 0.0,
            },
        });
        assert_eq!(outside, RepairObservationDecision::OutsideRegion);

        let observation = RepairObservation {
            observation_id: "same".into(),
            position_session_meters: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
            evidence: RepairConfidence {
                geometry: 0.4,
                texture: 0.4,
                material: 0.0,
            },
        };
        assert!(matches!(
            tracker.consider_observation(&observation),
            RepairObservationDecision::Accepted { .. }
        ));
        assert_eq!(
            tracker.consider_observation(&observation),
            RepairObservationDecision::DuplicateObservation
        );
    }

    #[test]
    fn completion_tracks_only_requested_dimensions() {
        let mut tracker = RepairTracker::new(
            region(RepairNeeds {
                geometry: true,
                texture: false,
                material: true,
            }),
            RepairConfidence {
                geometry: 0.7,
                texture: 0.0,
                material: 0.7,
            },
            0.8,
            0.0,
        )
        .unwrap();

        assert!(!tracker.status().complete);

        tracker.consider_observation(&RepairObservation {
            observation_id: "finish".into(),
            position_session_meters: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
            evidence: RepairConfidence {
                geometry: 0.5,
                texture: 0.0,
                material: 0.5,
            },
        });

        let status = tracker.status();
        assert!(status.complete);
        assert_eq!(status.progress, 1.0);
        assert_eq!(status.accepted_observation_count, 1);
    }
}
