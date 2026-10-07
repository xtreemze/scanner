use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    observation::RawObservation,
    reconstruction::{
        ReconstructionCheckpoint, ReconstructionRestoreError, ReconstructionVolume,
    },
    session::{ScanSession, SESSION_SCHEMA_VERSION},
    spatial::{
        SessionWorld, SessionWorldCheckpoint, SessionWorldRestoreError,
    },
};

pub const SESSION_CHECKPOINT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureSessionCheckpoint {
    pub checkpoint_schema_version: u32,
    pub session: ScanSession,
    pub raw_observations: Vec<RawObservation>,
    pub session_world: SessionWorldCheckpoint,
    pub reconstructions: Vec<ReconstructionCheckpoint>,
}

#[derive(Debug)]
pub struct RestoredSessionState {
    pub session: ScanSession,
    pub raw_observations: Vec<RawObservation>,
    pub session_world: SessionWorld,
    pub reconstructions: Vec<ReconstructionVolume>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionCheckpointError {
    UnsupportedCheckpointSchema { expected: u32, actual: u32 },
    UnsupportedSessionSchema { expected: u32, actual: u32 },
    SessionWorldEpochMismatch { session: u64, world: u64 },
    ObservationEpochMismatch {
        observation_id: String,
        session: u64,
        observation: u64,
    },
    DuplicateObservationId { observation_id: String },
    SessionWorldRestore(SessionWorldRestoreError),
    ReconstructionRestore {
        index: usize,
        error: ReconstructionRestoreError,
    },
}

impl CaptureSessionCheckpoint {
    pub fn capture(
        session: ScanSession,
        raw_observations: Vec<RawObservation>,
        session_world: &SessionWorld,
        reconstructions: &[ReconstructionVolume],
    ) -> Result<Self, SessionCheckpointError> {
        let checkpoint = Self {
            checkpoint_schema_version: SESSION_CHECKPOINT_SCHEMA_VERSION,
            session,
            raw_observations,
            session_world: session_world.checkpoint(),
            reconstructions: reconstructions
                .iter()
                .map(ReconstructionVolume::checkpoint)
                .collect(),
        };
        checkpoint.validate()?;
        Ok(checkpoint)
    }

    pub fn validate(&self) -> Result<(), SessionCheckpointError> {
        if self.checkpoint_schema_version != SESSION_CHECKPOINT_SCHEMA_VERSION {
            return Err(SessionCheckpointError::UnsupportedCheckpointSchema {
                expected: SESSION_CHECKPOINT_SCHEMA_VERSION,
                actual: self.checkpoint_schema_version,
            });
        }

        if self.session.schema_version != SESSION_SCHEMA_VERSION {
            return Err(SessionCheckpointError::UnsupportedSessionSchema {
                expected: SESSION_SCHEMA_VERSION,
                actual: self.session.schema_version,
            });
        }

        if self.session.epoch != self.session_world.epoch {
            return Err(SessionCheckpointError::SessionWorldEpochMismatch {
                session: self.session.epoch,
                world: self.session_world.epoch,
            });
        }

        let mut observation_ids = BTreeSet::new();
        for observation in &self.raw_observations {
            if observation.epoch() != self.session.epoch {
                return Err(SessionCheckpointError::ObservationEpochMismatch {
                    observation_id: observation.id().to_owned(),
                    session: self.session.epoch,
                    observation: observation.epoch(),
                });
            }
            if !observation_ids.insert(observation.id().to_owned()) {
                return Err(SessionCheckpointError::DuplicateObservationId {
                    observation_id: observation.id().to_owned(),
                });
            }
        }

        Ok(())
    }

    pub fn restore(self) -> Result<RestoredSessionState, SessionCheckpointError> {
        self.validate()?;

        let session_world = SessionWorld::from_checkpoint(self.session_world)
            .map_err(SessionCheckpointError::SessionWorldRestore)?;

        let mut reconstructions = Vec::with_capacity(self.reconstructions.len());
        for (index, checkpoint) in self.reconstructions.into_iter().enumerate() {
            let volume = ReconstructionVolume::from_checkpoint(checkpoint)
                .map_err(|error| SessionCheckpointError::ReconstructionRestore {
                    index,
                    error,
                })?;
            reconstructions.push(volume);
        }

        Ok(RestoredSessionState {
            session: self.session,
            raw_observations: self.raw_observations,
            session_world,
            reconstructions,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        observation::{
            ClockDomain, ImuObservation, ObservationEnvelope, ObservationSource,
            Timestamp, Vec3,
        },
        reconstruction::{FusionConfig, ReconstructionFrame},
    };

    fn imu(id: &str, epoch: u64) -> RawObservation {
        RawObservation::Imu(ObservationEnvelope {
            id: id.into(),
            device_id: "phone-a".into(),
            epoch,
            captured_at: Timestamp {
                micros: 100,
                clock_domain: ClockDomain::SessionMonotonic,
                uncertainty_micros: 10,
            },
            source: ObservationSource::Imu,
            payload: ImuObservation {
                acceleration_mps2: Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 9.81,
                },
                angular_velocity_rps: Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                gravity_mps2: None,
            },
        })
    }

    #[test]
    fn capture_and_restore_keep_raw_evidence_world_and_reconstruction() {
        let session = ScanSession::new("scan-a", 5);
        let world = SessionWorld::new(5);
        let reconstruction = ReconstructionVolume::new(
            FusionConfig {
                voxel_size_meters: 0.1,
                max_voxels: 100,
                max_sample_weight: 10_000.0,
            },
            ReconstructionFrame::SessionWorld,
        )
        .unwrap();

        let checkpoint = CaptureSessionCheckpoint::capture(
            session,
            vec![imu("imu-1", 5)],
            &world,
            &[reconstruction],
        )
        .unwrap();

        let restored = checkpoint.restore().unwrap();
        assert_eq!(restored.session.id, "scan-a");
        assert_eq!(restored.session.epoch, 5);
        assert_eq!(restored.raw_observations.len(), 1);
        assert_eq!(restored.session_world.epoch(), 5);
        assert_eq!(restored.reconstructions.len(), 1);
    }

    #[test]
    fn checkpoint_rejects_observation_from_another_epoch() {
        let session = ScanSession::new("scan-a", 5);
        let world = SessionWorld::new(5);

        assert_eq!(
            CaptureSessionCheckpoint::capture(
                session,
                vec![imu("stale", 4)],
                &world,
                &[],
            ),
            Err(SessionCheckpointError::ObservationEpochMismatch {
                observation_id: "stale".into(),
                session: 5,
                observation: 4,
            })
        );
    }

    #[test]
    fn checkpoint_rejects_duplicate_observation_identity() {
        let session = ScanSession::new("scan-a", 5);
        let world = SessionWorld::new(5);

        assert_eq!(
            CaptureSessionCheckpoint::capture(
                session,
                vec![imu("same", 5), imu("same", 5)],
                &world,
                &[],
            ),
            Err(SessionCheckpointError::DuplicateObservationId {
                observation_id: "same".into(),
            })
        );
    }

    #[test]
    fn checkpoint_rejects_world_from_another_epoch() {
        let session = ScanSession::new("scan-a", 5);
        let world = SessionWorld::new(6);

        assert_eq!(
            CaptureSessionCheckpoint::capture(session, vec![], &world, &[]),
            Err(SessionCheckpointError::SessionWorldEpochMismatch {
                session: 5,
                world: 6,
            })
        );
    }

    #[test]
    fn restore_rejects_unknown_checkpoint_schema() {
        let mut checkpoint = CaptureSessionCheckpoint::capture(
            ScanSession::new("scan-a", 1),
            vec![],
            &SessionWorld::new(1),
            &[],
        )
        .unwrap();
        checkpoint.checkpoint_schema_version = SESSION_CHECKPOINT_SCHEMA_VERSION + 1;

        assert!(matches!(
            checkpoint.restore(),
            Err(SessionCheckpointError::UnsupportedCheckpointSchema { .. })
        ));
    }
}
