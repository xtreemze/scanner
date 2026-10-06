use serde::{Deserialize, Serialize};

use crate::observation::Vec3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StructuralSurfaceKind {
    Floor,
    Wall,
    Ceiling,
    Surface,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LockState {
    Candidate,
    Stable,
    SuggestedLock,
    UserConfirmed,
    Locked,
    Challenged,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructuralSurface {
    pub id: String,
    pub kind: StructuralSurfaceKind,
    pub normal: Vec3,
    pub offset_meters: f64,
    pub confidence: f32,
    pub lock_state: LockState,
    pub supporting_observations: u32,
    pub contradictory_observations: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceEvent {
    EvidenceAccumulated,
    SuggestLock,
    UserConfirmed,
    Lock,
    Contradiction,
    Revalidated,
    Unlock,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceTransitionError {
    InvalidTransition { from: LockState, event: SurfaceEvent },
}

impl StructuralSurface {
    pub fn apply(&mut self, event: SurfaceEvent) -> Result<(), SurfaceTransitionError> {
        let next = match (self.lock_state, event) {
            (LockState::Candidate, SurfaceEvent::EvidenceAccumulated) => LockState::Stable,
            (LockState::Stable, SurfaceEvent::SuggestLock) => LockState::SuggestedLock,
            (LockState::SuggestedLock, SurfaceEvent::UserConfirmed) => LockState::UserConfirmed,
            (LockState::UserConfirmed, SurfaceEvent::Lock) => LockState::Locked,
            (LockState::Locked, SurfaceEvent::Contradiction) => LockState::Challenged,
            (LockState::Challenged, SurfaceEvent::Revalidated) => LockState::Locked,
            (_, SurfaceEvent::Unlock) => LockState::Stable,
            (from, event) => {
                return Err(SurfaceTransitionError::InvalidTransition { from, event });
            }
        };

        match event {
            SurfaceEvent::EvidenceAccumulated | SurfaceEvent::Revalidated => {
                self.supporting_observations = self.supporting_observations.saturating_add(1);
            }
            SurfaceEvent::Contradiction => {
                self.contradictory_observations =
                    self.contradictory_observations.saturating_add(1);
            }
            _ => {}
        }

        self.lock_state = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surface() -> StructuralSurface {
        StructuralSurface {
            id: "wall-west".into(),
            kind: StructuralSurfaceKind::Wall,
            normal: Vec3 { x: 1.0, y: 0.0, z: 0.0 },
            offset_meters: 2.0,
            confidence: 0.8,
            lock_state: LockState::Candidate,
            supporting_observations: 0,
            contradictory_observations: 0,
        }
    }

    #[test]
    fn lock_requires_explicit_user_confirmation_path() {
        let mut s = surface();
        s.apply(SurfaceEvent::EvidenceAccumulated).unwrap();
        s.apply(SurfaceEvent::SuggestLock).unwrap();
        assert_eq!(s.apply(SurfaceEvent::Lock), Err(SurfaceTransitionError::InvalidTransition {
            from: LockState::SuggestedLock,
            event: SurfaceEvent::Lock,
        }));
        s.apply(SurfaceEvent::UserConfirmed).unwrap();
        s.apply(SurfaceEvent::Lock).unwrap();
        assert_eq!(s.lock_state, LockState::Locked);
    }

    #[test]
    fn locked_surface_can_be_challenged_and_revalidated() {
        let mut s = surface();
        s.lock_state = LockState::Locked;
        s.apply(SurfaceEvent::Contradiction).unwrap();
        assert_eq!(s.lock_state, LockState::Challenged);
        assert_eq!(s.contradictory_observations, 1);
        s.apply(SurfaceEvent::Revalidated).unwrap();
        assert_eq!(s.lock_state, LockState::Locked);
    }

    #[test]
    fn unlock_never_discards_measured_geometry() {
        let mut s = surface();
        s.lock_state = LockState::Locked;
        let before = (s.normal, s.offset_meters);
        s.apply(SurfaceEvent::Unlock).unwrap();
        assert_eq!(s.lock_state, LockState::Stable);
        assert_eq!((s.normal, s.offset_meters), before);
    }
}
