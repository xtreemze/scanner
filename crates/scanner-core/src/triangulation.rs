//! Deterministic two-view triangulation after both observations have been
//! transformed into the canonical SessionWorld coordinate frame.
//!
//! This module solves geometry only: callers must establish time alignment,
//! visual feature correspondence, camera intrinsics/extrinsics, and per-ray
//! uncertainty before admitting any result as an authoritative constraint.

use crate::observation::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SessionRay {
    pub origin_session_meters: Vec3,
    pub direction_session: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TriangulationConfig {
    pub min_parallax_degrees: f64,
    pub max_ray_miss_meters: f64,
    pub min_range_meters: f64,
    pub max_range_meters: f64,
}

impl Default for TriangulationConfig {
    fn default() -> Self {
        Self {
            min_parallax_degrees: 2.0,
            max_ray_miss_meters: 0.05,
            min_range_meters: 0.05,
            max_range_meters: 20.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriangulationFailure {
    InvalidConfig,
    InvalidRay,
    InsufficientParallax,
    BehindCamera,
    OutOfRange,
    ExcessiveRayMiss,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TriangulatedPoint {
    pub position_session_meters: Vec3,
    pub first_range_meters: f64,
    pub second_range_meters: f64,
    pub parallax_degrees: f64,
    /// Closest-approach distance between the two rays, not metric ground-truth error.
    pub ray_miss_meters: f64,
}

pub fn triangulate_session_rays(
    first: SessionRay,
    second: SessionRay,
    config: TriangulationConfig,
) -> Result<TriangulatedPoint, TriangulationFailure> {
    if !config.min_parallax_degrees.is_finite()
        || !(0.0..90.0).contains(&config.min_parallax_degrees)
        || !config.max_ray_miss_meters.is_finite()
        || config.max_ray_miss_meters < 0.0
        || !config.min_range_meters.is_finite()
        || config.min_range_meters < 0.0
        || !config.max_range_meters.is_finite()
        || config.max_range_meters <= config.min_range_meters
    {
        return Err(TriangulationFailure::InvalidConfig);
    }

    if !finite(first.origin_session_meters) || !finite(second.origin_session_meters) {
        return Err(TriangulationFailure::InvalidRay);
    }
    let u = unit(first.direction_session).ok_or(TriangulationFailure::InvalidRay)?;
    let v = unit(second.direction_session).ok_or(TriangulationFailure::InvalidRay)?;
    let cosine = dot(u, v).clamp(-1.0, 1.0);
    let parallax_degrees = cosine.abs().acos().to_degrees();
    let denominator = 1.0 - cosine * cosine;
    if parallax_degrees < config.min_parallax_degrees || denominator <= 1e-12 {
        return Err(TriangulationFailure::InsufficientParallax);
    }

    let w = sub(first.origin_session_meters, second.origin_session_meters);
    let d = dot(u, w);
    let e = dot(v, w);
    let first_range = (cosine * e - d) / denominator;
    let second_range = (e - cosine * d) / denominator;
    if !first_range.is_finite() || !second_range.is_finite() {
        return Err(TriangulationFailure::InvalidRay);
    }
    if first_range <= 0.0 || second_range <= 0.0 {
        return Err(TriangulationFailure::BehindCamera);
    }
    if first_range < config.min_range_meters
        || second_range < config.min_range_meters
        || first_range > config.max_range_meters
        || second_range > config.max_range_meters
    {
        return Err(TriangulationFailure::OutOfRange);
    }

    let p = add(first.origin_session_meters, scale(u, first_range));
    let q = add(second.origin_session_meters, scale(v, second_range));
    let ray_miss_meters = length(sub(p, q));
    if !ray_miss_meters.is_finite() || ray_miss_meters > config.max_ray_miss_meters {
        return Err(TriangulationFailure::ExcessiveRayMiss);
    }
    let point = scale(add(p, q), 0.5);
    if !finite(point) {
        return Err(TriangulationFailure::InvalidRay);
    }
    Ok(TriangulatedPoint {
        position_session_meters: point,
        first_range_meters: first_range,
        second_range_meters: second_range,
        parallax_degrees,
        ray_miss_meters,
    })
}

fn finite(v: Vec3) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.z.is_finite()
}
fn dot(a: Vec3, b: Vec3) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}
fn length(v: Vec3) -> f64 {
    dot(v, v).sqrt()
}
fn unit(v: Vec3) -> Option<Vec3> {
    if !finite(v) {
        return None;
    }
    let norm = length(v);
    (norm.is_finite() && norm > 1e-12).then(|| scale(v, 1.0 / norm))
}
fn sub(a: Vec3, b: Vec3) -> Vec3 {
    Vec3 { x: a.x - b.x, y: a.y - b.y, z: a.z - b.z }
}
fn add(a: Vec3, b: Vec3) -> Vec3 {
    Vec3 { x: a.x + b.x, y: a.y + b.y, z: a.z + b.z }
}
fn scale(a: Vec3, n: f64) -> Vec3 {
    Vec3 { x: a.x * n, y: a.y * n, z: a.z * n }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f64, y: f64, z: f64) -> Vec3 { Vec3 { x, y, z } }
    fn ray(origin: Vec3, direction: Vec3) -> SessionRay {
        SessionRay { origin_session_meters: origin, direction_session: direction }
    }

    #[test]
    fn reconstructs_metric_point_from_converging_rays() {
        let point = triangulate_session_rays(
            ray(v(0.0, 0.0, 0.0), v(0.5, 0.0, 2.0)),
            ray(v(1.0, 0.0, 0.0), v(-0.5, 0.0, 2.0)),
            TriangulationConfig::default(),
        ).unwrap();
        assert!((point.position_session_meters.x - 0.5).abs() < 1e-9);
        assert!((point.position_session_meters.z - 2.0).abs() < 1e-9);
        assert!(point.ray_miss_meters < 1e-9);
        assert!(point.parallax_degrees > 20.0);
    }

    #[test]
    fn rejects_parallel_and_zero_baseline_geometry() {
        assert_eq!(
            triangulate_session_rays(
                ray(v(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0)),
                ray(v(1.0, 0.0, 0.0), v(0.0, 0.0, 1.0)),
                TriangulationConfig::default(),
            ),
            Err(TriangulationFailure::InsufficientParallax)
        );
        assert_eq!(
            triangulate_session_rays(
                ray(v(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0)),
                ray(v(0.0, 0.0, 0.0), v(0.5, 0.0, 1.0)),
                TriangulationConfig::default(),
            ),
            Err(TriangulationFailure::BehindCamera)
        );
    }

    #[test]
    fn rejects_points_behind_one_camera() {
        assert_eq!(
            triangulate_session_rays(
                ray(v(0.0, 0.0, 0.0), v(0.5, 0.0, 2.0)),
                ray(v(1.0, 0.0, 0.0), v(0.5, 0.0, -2.0)),
                TriangulationConfig::default(),
            ),
            Err(TriangulationFailure::BehindCamera)
        );
    }

    #[test]
    fn rejects_inconsistent_correspondences() {
        assert_eq!(
            triangulate_session_rays(
                ray(v(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0)),
                ray(v(1.0, 0.0, 0.0), v(-1.0, 1.0, 2.0)),
                TriangulationConfig::default(),
            ),
            Err(TriangulationFailure::ExcessiveRayMiss)
        );
    }

    #[test]
    fn validates_bounds_and_nonfinite_inputs() {
        let first = ray(v(0.0, 0.0, 0.0), v(0.5, 0.0, 2.0));
        let second = ray(v(1.0, 0.0, 0.0), v(-0.5, 0.0, 2.0));
        assert_eq!(
            triangulate_session_rays(first, second, TriangulationConfig {
                min_parallax_degrees: 90.0,
                ..TriangulationConfig::default()
            }),
            Err(TriangulationFailure::InvalidConfig)
        );
        assert_eq!(
            triangulate_session_rays(ray(v(f64::NAN, 0.0, 0.0), v(1.0, 0.0, 0.0)), second,
                TriangulationConfig::default()),
            Err(TriangulationFailure::InvalidRay)
        );
        assert_eq!(
            triangulate_session_rays(first, second, TriangulationConfig {
                max_range_meters: 1.0,
                ..TriangulationConfig::default()
            }),
            Err(TriangulationFailure::OutOfRange)
        );
    }
}
