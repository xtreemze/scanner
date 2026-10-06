use serde::{Deserialize, Serialize};

use crate::{
    guidance::ConfidenceField,
    structural::StructuralSurface,
};

pub const SESSION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSession {
    pub schema_version: u32,
    pub id: String,
    pub epoch: u64,
    pub structural_surfaces: Vec<StructuralSurface>,
    pub confidence: ConfidenceField,
}

impl ScanSession {
    pub fn new(id: impl Into<String>, epoch: u64) -> Self {
        Self {
            schema_version: SESSION_SCHEMA_VERSION,
            id: id.into(),
            epoch,
            structural_surfaces: Vec::new(),
            confidence: ConfidenceField {
                geometry: 0.0,
                pose: 0.0,
                coverage: 0.0,
                view_angle_diversity: 0.0,
                texture: 0.0,
                material: 0.0,
                structural: 0.0,
                peer_geometry: 0.0,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_generation_is_explicit_and_versioned() {
        let session = ScanSession::new("scan-1", 7);
        assert_eq!(session.schema_version, SESSION_SCHEMA_VERSION);
        assert_eq!(session.epoch, 7);
    }
}
