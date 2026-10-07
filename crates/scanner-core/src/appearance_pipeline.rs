use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    appearance::{ControlledIlluminationPair, MaterialEstimateError, PairingThresholds, PbrMaterialEstimate},
    observation::Vec3,
};

const DEFAULT_AZIMUTH_BINS: usize = 16;
const DEFAULT_ELEVATION_BINS: usize = 8;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HdrEnvironmentSample {
    pub frame_id: String,
    pub direction_session: Vec3,
    pub linear_rgb: [f32; 3],
    pub relative_exposure_ev: f32,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HdrSampleError {
    MissingFrameIdentity,
    InvalidDirection,
    InvalidRadiance,
    InvalidExposure,
    InvalidConfidence,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnvironmentRadianceBin {
    pub bin_index: usize,
    pub radiance_linear_rgb: [f32; 3],
    pub confidence: f32,
    pub exposure_span_ev: f32,
    pub frame_ids: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct HdrBinAccumulator {
    weighted_rgb: [f64; 3],
    total_weight: f64,
    min_ev: f32,
    max_ev: f32,
    frame_ids: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub struct HdrEnvironmentAccumulator {
    bins: Vec<HdrBinAccumulator>,
    azimuth_bins: usize,
    elevation_bins: usize,
}

impl Default for HdrEnvironmentAccumulator {
    fn default() -> Self {
        Self::new(DEFAULT_AZIMUTH_BINS, DEFAULT_ELEVATION_BINS)
    }
}

impl HdrEnvironmentAccumulator {
    pub fn new(azimuth_bins: usize, elevation_bins: usize) -> Self {
        let azimuth_bins = azimuth_bins.max(1);
        let elevation_bins = elevation_bins.max(1);
        Self {
            bins: vec![HdrBinAccumulator::default(); azimuth_bins * elevation_bins],
            azimuth_bins,
            elevation_bins,
        }
    }

    pub fn observe(&mut self, sample: HdrEnvironmentSample) -> Result<(), HdrSampleError> {
        validate_hdr_sample(&sample)?;
        let direction = normalize(sample.direction_session).ok_or(HdrSampleError::InvalidDirection)?;
        let index = direction_bin(direction, self.azimuth_bins, self.elevation_bins);

        let exposure_scale = 2.0f64.powf(-(sample.relative_exposure_ev as f64));
        let weight = sample.confidence as f64;
        let bin = &mut self.bins[index];

        for channel in 0..3 {
            bin.weighted_rgb[channel] +=
                sample.linear_rgb[channel] as f64 * exposure_scale * weight;
        }
        bin.total_weight += weight;
        if bin.frame_ids.is_empty() {
            bin.min_ev = sample.relative_exposure_ev;
            bin.max_ev = sample.relative_exposure_ev;
        } else {
            bin.min_ev = bin.min_ev.min(sample.relative_exposure_ev);
            bin.max_ev = bin.max_ev.max(sample.relative_exposure_ev);
        }
        bin.frame_ids.insert(sample.frame_id);
        Ok(())
    }

    pub fn coverage_fraction(&self) -> f32 {
        let occupied = self.bins.iter().filter(|bin| bin.total_weight > 0.0).count();
        occupied as f32 / self.bins.len() as f32
    }

    pub fn radiance_bins(&self) -> Vec<EnvironmentRadianceBin> {
        self.bins
            .iter()
            .enumerate()
            .filter_map(|(index, bin)| {
                if bin.total_weight <= 0.0 {
                    return None;
                }
                let exposure_span_ev = bin.max_ev - bin.min_ev;
                let bracket_quality = (exposure_span_ev / 4.0).clamp(0.0, 1.0);
                let observation_quality = (bin.total_weight as f32 / 3.0).clamp(0.0, 1.0);
                Some(EnvironmentRadianceBin {
                    bin_index: index,
                    radiance_linear_rgb: [
                        (bin.weighted_rgb[0] / bin.total_weight) as f32,
                        (bin.weighted_rgb[1] / bin.total_weight) as f32,
                        (bin.weighted_rgb[2] / bin.total_weight) as f32,
                    ],
                    confidence: (0.65 * observation_quality + 0.35 * bracket_quality)
                        .clamp(0.0, 1.0),
                    exposure_span_ev,
                    frame_ids: bin.frame_ids.iter().cloned().collect(),
                })
            })
            .collect()
    }

    pub fn next_missing_direction(&self) -> Option<Vec3> {
        self.bins
            .iter()
            .position(|bin| bin.total_weight <= 0.0)
            .map(|index| bin_center(index, self.azimuth_bins, self.elevation_bins))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IlluminationEmitter {
    pub device_id: String,
    pub position_session: Option<Vec3>,
    pub available: bool,
    pub calibration_confidence: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IlluminationStepKind {
    AmbientReference,
    FlashPulse,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IlluminationStep {
    pub sequence_index: usize,
    pub kind: IlluminationStepKind,
    pub capture_device_id: String,
    pub emitter_device_id: Option<String>,
    pub settle_millis: u32,
    pub emitter_position_session: Option<Vec3>,
    pub calibration_confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IlluminationPlanError {
    MissingCaptureDevice,
    InvalidEmitter,
    NoAvailableEmitter,
}

pub fn plan_illumination_sequence(
    capture_device_id: &str,
    emitters: &[IlluminationEmitter],
) -> Result<Vec<IlluminationStep>, IlluminationPlanError> {
    if capture_device_id.is_empty() {
        return Err(IlluminationPlanError::MissingCaptureDevice);
    }

    let mut available = emitters
        .iter()
        .filter(|emitter| emitter.available && emitter.device_id != capture_device_id)
        .cloned()
        .collect::<Vec<_>>();

    for emitter in &available {
        if emitter.device_id.is_empty()
            || !emitter.calibration_confidence.is_finite()
            || !(0.0..=1.0).contains(&emitter.calibration_confidence)
            || emitter.position_session.is_some_and(|position| !vec_is_finite(position))
        {
            return Err(IlluminationPlanError::InvalidEmitter);
        }
    }

    if available.is_empty() {
        return Err(IlluminationPlanError::NoAvailableEmitter);
    }

    available.sort_by(|a, b| a.device_id.cmp(&b.device_id));

    let mut steps = vec![IlluminationStep {
        sequence_index: 0,
        kind: IlluminationStepKind::AmbientReference,
        capture_device_id: capture_device_id.to_owned(),
        emitter_device_id: None,
        settle_millis: 150,
        emitter_position_session: None,
        calibration_confidence: 1.0,
    }];

    for (offset, emitter) in available.into_iter().enumerate() {
        steps.push(IlluminationStep {
            sequence_index: offset + 1,
            kind: IlluminationStepKind::FlashPulse,
            capture_device_id: capture_device_id.to_owned(),
            emitter_device_id: Some(emitter.device_id),
            settle_millis: 120,
            emitter_position_session: emitter.position_session,
            calibration_confidence: emitter.calibration_confidence,
        });
    }

    Ok(steps)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextureViewCandidate {
    pub frame_id: String,
    pub projected_texels_per_meter: f32,
    pub incidence_cosine: f32,
    pub blur_penalty: f32,
    pub exposure_quality: f32,
    pub pose_confidence: f32,
    pub occlusion_fraction: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextureViewError {
    InvalidCandidate,
    MissingIdentity,
}

impl TextureViewCandidate {
    pub fn validate(&self) -> Result<(), TextureViewError> {
        if self.frame_id.is_empty() {
            return Err(TextureViewError::MissingIdentity);
        }
        if !self.projected_texels_per_meter.is_finite()
            || self.projected_texels_per_meter < 0.0
            || !unit(self.incidence_cosine)
            || !unit(self.blur_penalty)
            || !unit(self.exposure_quality)
            || !unit(self.pose_confidence)
            || !unit(self.occlusion_fraction)
        {
            return Err(TextureViewError::InvalidCandidate);
        }
        Ok(())
    }

    pub fn score(&self) -> Result<f32, TextureViewError> {
        self.validate()?;
        let density = (self.projected_texels_per_meter / 1500.0).clamp(0.0, 1.0);
        let visibility = 1.0 - self.occlusion_fraction;
        let sharpness = 1.0 - self.blur_penalty;
        Ok((0.30 * density
            + 0.22 * self.incidence_cosine
            + 0.18 * sharpness
            + 0.12 * self.exposure_quality
            + 0.18 * self.pose_confidence)
            * visibility)
    }
}

pub fn choose_texture_view(
    candidates: &[TextureViewCandidate],
) -> Result<Option<&TextureViewCandidate>, TextureViewError> {
    let mut best: Option<(&TextureViewCandidate, f32)> = None;
    for candidate in candidates {
        let score = candidate.score()?;
        match best {
            Some((_, best_score)) if score <= best_score => {}
            _ => best = Some((candidate, score)),
        }
    }
    Ok(best.map(|(candidate, _)| candidate))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotometricRegionStats {
    pub ambient_mean_linear_rgb: [f32; 3],
    pub illuminated_mean_linear_rgb: [f32; 3],
    pub illuminated_peak_linear_rgb: [f32; 3],
    pub sample_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialPhotometricEvidence {
    pub pair_id: String,
    pub diffuse_response_linear_rgb: [f32; 3],
    pub specular_excess_linear_rgb: [f32; 3],
    pub base_color_linear_rgb: [f32; 3],
    pub emitter_pose_known: bool,
    pub calibration_confidence: f32,
    pub sample_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaterialEvidenceError {
    InvalidPair,
    InvalidStatistics,
    InvalidCalibration,
}

pub fn derive_material_evidence(
    pair_id: impl Into<String>,
    pair: &ControlledIlluminationPair,
    stats: PhotometricRegionStats,
    calibration_confidence: f32,
) -> Result<MaterialPhotometricEvidence, MaterialEvidenceError> {
    pair.validate(PairingThresholds::default())
        .map_err(|_| MaterialEvidenceError::InvalidPair)?;

    if !unit(calibration_confidence) {
        return Err(MaterialEvidenceError::InvalidCalibration);
    }
    if stats.sample_count == 0
        || !rgb_nonnegative_finite(stats.ambient_mean_linear_rgb)
        || !rgb_nonnegative_finite(stats.illuminated_mean_linear_rgb)
        || !rgb_nonnegative_finite(stats.illuminated_peak_linear_rgb)
    {
        return Err(MaterialEvidenceError::InvalidStatistics);
    }

    let diffuse_response_linear_rgb = positive_subtract(
        stats.illuminated_mean_linear_rgb,
        stats.ambient_mean_linear_rgb,
    );
    let specular_excess_linear_rgb = positive_subtract(
        stats.illuminated_peak_linear_rgb,
        stats.illuminated_mean_linear_rgb,
    );

    Ok(MaterialPhotometricEvidence {
        pair_id: pair_id.into(),
        diffuse_response_linear_rgb,
        specular_excess_linear_rgb,
        base_color_linear_rgb: stats.ambient_mean_linear_rgb,
        emitter_pose_known: pair.has_known_emitter_pose(),
        calibration_confidence,
        sample_count: stats.sample_count,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct MaterialInference {
    pub estimate: PbrMaterialEstimate,
    pub diffuse_evidence_strength: f32,
    pub specular_evidence_strength: f32,
    pub calibrated_fraction: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaterialInferenceError {
    MissingEvidence,
    InvalidEvidence,
    InvalidEstimate(MaterialEstimateError),
}

pub fn infer_pbr_material(
    evidence: &[MaterialPhotometricEvidence],
) -> Result<MaterialInference, MaterialInferenceError> {
    if evidence.is_empty() {
        return Err(MaterialInferenceError::MissingEvidence);
    }

    let mut total_weight = 0.0f32;
    let mut base = [0.0f32; 3];
    let mut diffuse_strength = 0.0f32;
    let mut specular_strength = 0.0f32;
    let mut calibrated_weight = 0.0f32;
    let mut evidence_ids = BTreeSet::new();

    for item in evidence {
        if item.pair_id.is_empty()
            || !rgb_nonnegative_finite(item.diffuse_response_linear_rgb)
            || !rgb_nonnegative_finite(item.specular_excess_linear_rgb)
            || !rgb_nonnegative_finite(item.base_color_linear_rgb)
            || !unit(item.calibration_confidence)
            || item.sample_count == 0
        {
            return Err(MaterialInferenceError::InvalidEvidence);
        }

        let sample_weight = (item.sample_count as f32).sqrt().min(64.0) / 8.0;
        let weight = (0.25 + 0.75 * item.calibration_confidence) * sample_weight;
        total_weight += weight;
        for channel in 0..3 {
            base[channel] += item.base_color_linear_rgb[channel] * weight;
        }

        let diffuse = luminance(item.diffuse_response_linear_rgb);
        let specular = luminance(item.specular_excess_linear_rgb);
        diffuse_strength += diffuse * weight;
        specular_strength += specular * weight;
        if item.emitter_pose_known {
            calibrated_weight += weight * item.calibration_confidence;
        }
        evidence_ids.insert(item.pair_id.clone());
    }

    if total_weight <= 0.0 || !total_weight.is_finite() {
        return Err(MaterialInferenceError::InvalidEvidence);
    }

    for channel in &mut base {
        *channel = (*channel / total_weight).clamp(0.0, 1.0);
    }
    diffuse_strength /= total_weight;
    specular_strength /= total_weight;

    let response_total = diffuse_strength + specular_strength + 1e-6;
    let specular_fraction = (specular_strength / response_total).clamp(0.0, 1.0);
    let roughness = (0.92 - 0.72 * specular_fraction).clamp(0.08, 1.0);

    // Phone flash observations are not sufficient to assert metallic behavior strongly.
    // Keep metallic conservative and bound it further when illumination geometry is weak.
    let base_saturation = saturation(base);
    let specular_saturation = average_saturation(
        evidence.iter().map(|item| item.specular_excess_linear_rgb),
    );
    let calibrated_fraction = (calibrated_weight / total_weight).clamp(0.0, 1.0);
    let metallic = ((specular_saturation - base_saturation).max(0.0)
        * 0.35
        * calibrated_fraction)
        .clamp(0.0, 0.35);

    let evidence_count_confidence = (evidence.len() as f32 / 5.0).clamp(0.0, 1.0);
    let response_confidence = (response_total / 0.25).clamp(0.0, 1.0);
    let raw_confidence =
        0.35 * evidence_count_confidence + 0.35 * response_confidence + 0.30 * calibrated_fraction;
    let confidence_cap = if calibrated_fraction >= 0.8 { 0.78 } else { 0.60 };
    let confidence = raw_confidence.min(confidence_cap);

    let estimate = PbrMaterialEstimate {
        base_color_linear_rgba: [base[0], base[1], base[2], 1.0],
        metallic,
        roughness,
        confidence,
        evidence_ids: evidence_ids.into_iter().collect(),
    };
    estimate
        .validate()
        .map_err(MaterialInferenceError::InvalidEstimate)?;

    Ok(MaterialInference {
        estimate,
        diffuse_evidence_strength: diffuse_strength,
        specular_evidence_strength: specular_strength,
        calibrated_fraction,
    })
}

fn validate_hdr_sample(sample: &HdrEnvironmentSample) -> Result<(), HdrSampleError> {
    if sample.frame_id.is_empty() {
        return Err(HdrSampleError::MissingFrameIdentity);
    }
    if normalize(sample.direction_session).is_none() {
        return Err(HdrSampleError::InvalidDirection);
    }
    if !rgb_nonnegative_finite(sample.linear_rgb) {
        return Err(HdrSampleError::InvalidRadiance);
    }
    if !sample.relative_exposure_ev.is_finite() {
        return Err(HdrSampleError::InvalidExposure);
    }
    if !unit(sample.confidence) {
        return Err(HdrSampleError::InvalidConfidence);
    }
    Ok(())
}

fn direction_bin(direction: Vec3, azimuth_bins: usize, elevation_bins: usize) -> usize {
    let azimuth = direction.z.atan2(direction.x);
    let elevation = direction.y.clamp(-1.0, 1.0).asin();
    let azimuth_unit =
        (azimuth + std::f64::consts::PI) / (2.0 * std::f64::consts::PI);
    let elevation_unit =
        (elevation + std::f64::consts::FRAC_PI_2) / std::f64::consts::PI;
    let azimuth_index =
        ((azimuth_unit * azimuth_bins as f64).floor() as usize).min(azimuth_bins - 1);
    let elevation_index =
        ((elevation_unit * elevation_bins as f64).floor() as usize).min(elevation_bins - 1);
    elevation_index * azimuth_bins + azimuth_index
}

fn bin_center(index: usize, azimuth_bins: usize, elevation_bins: usize) -> Vec3 {
    let elevation_index = index / azimuth_bins;
    let azimuth_index = index % azimuth_bins;

    let azimuth_unit = (azimuth_index as f64 + 0.5) / azimuth_bins as f64;
    let elevation_unit = (elevation_index as f64 + 0.5) / elevation_bins as f64;
    let azimuth = azimuth_unit * 2.0 * std::f64::consts::PI - std::f64::consts::PI;
    let elevation = elevation_unit * std::f64::consts::PI - std::f64::consts::FRAC_PI_2;
    let cos_elevation = elevation.cos();

    Vec3 {
        x: cos_elevation * azimuth.cos(),
        y: elevation.sin(),
        z: cos_elevation * azimuth.sin(),
    }
}

fn normalize(v: Vec3) -> Option<Vec3> {
    if !vec_is_finite(v) {
        return None;
    }
    let magnitude = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
    if magnitude <= 1e-12 {
        return None;
    }
    Some(Vec3 {
        x: v.x / magnitude,
        y: v.y / magnitude,
        z: v.z / magnitude,
    })
}

fn vec_is_finite(v: Vec3) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.z.is_finite()
}

fn unit(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn rgb_nonnegative_finite(rgb: [f32; 3]) -> bool {
    rgb.iter().all(|value| value.is_finite() && *value >= 0.0)
}

fn positive_subtract(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        (a[0] - b[0]).max(0.0),
        (a[1] - b[1]).max(0.0),
        (a[2] - b[2]).max(0.0),
    ]
}

fn luminance(rgb: [f32; 3]) -> f32 {
    0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
}

fn saturation(rgb: [f32; 3]) -> f32 {
    let max = rgb[0].max(rgb[1]).max(rgb[2]);
    let min = rgb[0].min(rgb[1]).min(rgb[2]);
    if max <= 1e-6 {
        0.0
    } else {
        ((max - min) / max).clamp(0.0, 1.0)
    }
}

fn average_saturation<I>(items: I) -> f32
where
    I: Iterator<Item = [f32; 3]>,
{
    let mut total = 0.0;
    let mut count = 0usize;
    for rgb in items {
        total += saturation(rgb);
        count += 1;
    }
    if count == 0 {
        0.0
    } else {
        total / count as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(known_pose: bool) -> ControlledIlluminationPair {
        ControlledIlluminationPair {
            ambient_frame_id: "ambient".into(),
            illuminated_frame_id: "flash".into(),
            illumination_observation_id: "illumination".into(),
            capture_device_id: "camera".into(),
            emitter_device_id: "light".into(),
            time_delta_micros: 20_000,
            camera_translation_delta_meters: 0.002,
            camera_rotation_delta_degrees: 0.2,
            exposure_delta_ev: 0.1,
            emitter_position_session: known_pose.then_some(Vec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            }),
        }
    }

    #[test]
    fn hdr_accumulator_combines_exposure_brackets_into_relative_radiance() {
        let mut hdr = HdrEnvironmentAccumulator::new(4, 2);
        let direction = Vec3 { x: 1.0, y: 0.0, z: 0.0 };
        hdr.observe(HdrEnvironmentSample {
            frame_id: "ev0".into(),
            direction_session: direction,
            linear_rgb: [0.5, 0.25, 0.125],
            relative_exposure_ev: 0.0,
            confidence: 1.0,
        }).unwrap();
        hdr.observe(HdrEnvironmentSample {
            frame_id: "ev1".into(),
            direction_session: direction,
            linear_rgb: [1.0, 0.5, 0.25],
            relative_exposure_ev: 1.0,
            confidence: 1.0,
        }).unwrap();

        let bins = hdr.radiance_bins();
        assert_eq!(bins.len(), 1);
        assert!((bins[0].radiance_linear_rgb[0] - 0.5).abs() < 1e-6);
        assert_eq!(bins[0].frame_ids, vec!["ev0", "ev1"]);
        assert_eq!(bins[0].exposure_span_ev, 1.0);
    }

    #[test]
    fn hdr_accumulator_reports_next_missing_direction() {
        let hdr = HdrEnvironmentAccumulator::new(2, 1);
        let missing = hdr.next_missing_direction().unwrap();
        let magnitude = (missing.x * missing.x + missing.y * missing.y + missing.z * missing.z).sqrt();
        assert!((magnitude - 1.0).abs() < 1e-9);
    }

    #[test]
    fn illumination_sequence_is_deterministic_and_starts_with_ambient_reference() {
        let steps = plan_illumination_sequence(
            "capture",
            &[
                IlluminationEmitter {
                    device_id: "z-light".into(),
                    position_session: None,
                    available: true,
                    calibration_confidence: 0.4,
                },
                IlluminationEmitter {
                    device_id: "a-light".into(),
                    position_session: Some(Vec3 { x: 1.0, y: 0.0, z: 0.0 }),
                    available: true,
                    calibration_confidence: 0.9,
                },
            ],
        ).unwrap();

        assert_eq!(steps[0].kind, IlluminationStepKind::AmbientReference);
        assert_eq!(steps[1].emitter_device_id.as_deref(), Some("a-light"));
        assert_eq!(steps[2].emitter_device_id.as_deref(), Some("z-light"));
    }

    #[test]
    fn texture_selection_prefers_visible_sharp_high_confidence_frame() {
        let weak = TextureViewCandidate {
            frame_id: "weak".into(),
            projected_texels_per_meter: 2000.0,
            incidence_cosine: 1.0,
            blur_penalty: 0.0,
            exposure_quality: 1.0,
            pose_confidence: 1.0,
            occlusion_fraction: 0.9,
        };
        let strong = TextureViewCandidate {
            frame_id: "strong".into(),
            projected_texels_per_meter: 1200.0,
            incidence_cosine: 0.85,
            blur_penalty: 0.05,
            exposure_quality: 0.9,
            pose_confidence: 0.95,
            occlusion_fraction: 0.05,
        };

        let candidates = vec![weak, strong.clone()];
        assert_eq!(choose_texture_view(&candidates).unwrap(), Some(&strong));
    }

    #[test]
    fn material_evidence_keeps_diffuse_and_specular_terms_separate() {
        let evidence = derive_material_evidence(
            "pair-a",
            &pair(true),
            PhotometricRegionStats {
                ambient_mean_linear_rgb: [0.2, 0.2, 0.2],
                illuminated_mean_linear_rgb: [0.5, 0.45, 0.4],
                illuminated_peak_linear_rgb: [0.9, 0.8, 0.7],
                sample_count: 256,
            },
            0.9,
        ).unwrap();

        assert_eq!(evidence.diffuse_response_linear_rgb, [0.3, 0.24999999, 0.2]);
        assert!(evidence.specular_excess_linear_rgb[0] > 0.39);
        assert!(evidence.emitter_pose_known);
    }

    #[test]
    fn pbr_inference_is_confidence_capped_when_emitter_pose_is_unknown() {
        let evidence = derive_material_evidence(
            "pair-a",
            &pair(false),
            PhotometricRegionStats {
                ambient_mean_linear_rgb: [0.3, 0.2, 0.1],
                illuminated_mean_linear_rgb: [0.55, 0.4, 0.25],
                illuminated_peak_linear_rgb: [0.8, 0.65, 0.4],
                sample_count: 400,
            },
            0.4,
        ).unwrap();

        let result = infer_pbr_material(&[evidence]).unwrap();
        assert!(result.estimate.confidence <= 0.60);
        assert_eq!(result.calibrated_fraction, 0.0);
        assert!(result.diffuse_evidence_strength > 0.0);
        assert!(result.specular_evidence_strength > 0.0);
    }

    #[test]
    fn pbr_inference_remains_conservative_about_metallicity() {
        let mut evidence = Vec::new();
        for index in 0..5 {
            evidence.push(
                derive_material_evidence(
                    format!("pair-{index}"),
                    &pair(true),
                    PhotometricRegionStats {
                        ambient_mean_linear_rgb: [0.2, 0.18, 0.16],
                        illuminated_mean_linear_rgb: [0.42, 0.37, 0.31],
                        illuminated_peak_linear_rgb: [0.95, 0.7, 0.4],
                        sample_count: 512,
                    },
                    1.0,
                ).unwrap(),
            );
        }

        let result = infer_pbr_material(&evidence).unwrap();
        assert!(result.estimate.confidence <= 0.78);
        assert!(result.estimate.metallic <= 0.35);
        assert!(result.estimate.roughness >= 0.08);
    }
}
