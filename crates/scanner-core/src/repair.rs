pub mod tracking;

use serde::{Deserialize, Serialize};

use crate::{
    observation::Vec3,
    reconstruction::{RaycastHit, SparseSurfaceVolume, VoxelKey},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairNeeds {
    pub geometry: bool,
    pub texture: bool,
    pub material: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairRegion {
    pub id: String,
    pub center_session_meters: Vec3,
    pub radius_meters: f64,
    pub source_voxel: VoxelKey,
    pub source_confidence: f32,
    pub needs: RepairNeeds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairSelectionError {
    InvalidRay,
    InvalidRadius,
    NoSurfaceHit,
}

pub fn select_repair_region(
    volume: &SparseSurfaceVolume,
    id: impl Into<String>,
    ray_origin_session_meters: Vec3,
    ray_direction_session: Vec3,
    min_surface_confidence: f32,
    radius_meters: f64,
    needs: RepairNeeds,
) -> Result<RepairRegion, RepairSelectionError> {
    if !vec_is_finite(ray_origin_session_meters)
        || !vec_is_finite(ray_direction_session)
        || vector_magnitude(ray_direction_session) <= 1e-12
    {
        return Err(RepairSelectionError::InvalidRay);
    }
    if !radius_meters.is_finite() || radius_meters <= 0.0 {
        return Err(RepairSelectionError::InvalidRadius);
    }

    let RaycastHit {
        voxel,
        position_session_meters,
        confidence,
        ..
    } = volume
        .raycast_surface(
            ray_origin_session_meters,
            ray_direction_session,
            min_surface_confidence,
        )
        .ok_or(RepairSelectionError::NoSurfaceHit)?;

    Ok(RepairRegion {
        id: id.into(),
        center_session_meters: position_session_meters,
        radius_meters,
        source_voxel: voxel,
        source_confidence: confidence,
        needs,
    })
}

impl RepairRegion {
    pub fn contains_session_point(&self, point: Vec3) -> bool {
        if !vec_is_finite(point) {
            return false;
        }

        let delta = Vec3 {
            x: point.x - self.center_session_meters.x,
            y: point.y - self.center_session_meters.y,
            z: point.z - self.center_session_meters.z,
        };
        vector_magnitude(delta) <= self.radius_meters
    }
}

fn vec_is_finite(v: Vec3) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.z.is_finite()
}

fn vector_magnitude(v: Vec3) -> f64 {
    (v.x * v.x + v.y * v.y + v.z * v.z).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        observation::{Pose, Quaternion},
        reconstruction::{FusionConfig, SurfaceSample},
    };

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

    fn volume_with_surface() -> SparseSurfaceVolume {
        let mut volume = SparseSurfaceVolume::new(FusionConfig {
            voxel_size_meters: 0.1,
            max_voxels: 10,
            max_sample_weight: 10_000.0,
        })
        .unwrap();

        volume.integrate_frame(
            identity_pose(),
            &[SurfaceSample {
                position_camera_meters: Vec3 {
                    x: 0.01,
                    y: 0.01,
                    z: 1.01,
                },
                confidence: 0.9,
                depth_uncertainty_meters: 0.01,
            }],
        );
        volume
    }

    #[test]
    fn live_ray_selection_becomes_registered_three_dimensional_region() {
        let volume = volume_with_surface();
        let region = select_repair_region(
            &volume,
            "repair-a",
            Vec3 {
                x: 0.01,
                y: 0.01,
                z: 0.0,
            },
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
            0.5,
            0.2,
            RepairNeeds {
                geometry: true,
                texture: false,
                material: false,
            },
        )
        .unwrap();

        assert_eq!(region.id, "repair-a");
        assert!(region.contains_session_point(Vec3 {
            x: region.center_session_meters.x,
            y: region.center_session_meters.y,
            z: region.center_session_meters.z + 0.1,
        }));
        assert!(!region.contains_session_point(Vec3 {
            x: region.center_session_meters.x,
            y: region.center_session_meters.y,
            z: region.center_session_meters.z + 0.3,
        }));
    }

    #[test]
    fn selection_fails_when_ray_misses_reconstruction() {
        let volume = volume_with_surface();
        assert_eq!(
            select_repair_region(
                &volume,
                "miss",
                Vec3 {
                    x: 10.0,
                    y: 10.0,
                    z: 0.0,
                },
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
                0.5,
                0.2,
                RepairNeeds {
                    geometry: true,
                    texture: true,
                    material: true,
                },
            ),
            Err(RepairSelectionError::NoSurfaceHit)
        );
    }

    #[test]
    fn invalid_radius_is_rejected_before_selection() {
        let volume = volume_with_surface();
        assert_eq!(
            select_repair_region(
                &volume,
                "invalid",
                Vec3 {
                    x: 0.01,
                    y: 0.01,
                    z: 0.0,
                },
                Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
                0.5,
                0.0,
                RepairNeeds {
                    geometry: true,
                    texture: false,
                    material: false,
                },
            ),
            Err(RepairSelectionError::InvalidRadius)
        );
    }
}
