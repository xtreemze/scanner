use crate::{
    appearance::{MaterialEstimateError, PbrMaterialEstimate},
    reconstruction::{PreviewMesh, ReconstructionVolume},
};

const GLB_MAGIC: u32 = 0x4654_6C67;
const GLB_VERSION: u32 = 2;
const JSON_CHUNK_TYPE: u32 = 0x4E4F_534A;
const BIN_CHUNK_TYPE: u32 = 0x004E_4942;
const ARRAY_BUFFER_TARGET: u32 = 34_962;
const ELEMENT_ARRAY_BUFFER_TARGET: u32 = 34_963;
const FLOAT_COMPONENT_TYPE: u32 = 5_126;
const UNSIGNED_INT_COMPONENT_TYPE: u32 = 5_125;
const TRIANGLES_MODE: u32 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlbExportError {
    EmptyMesh,
    NonTriangleIndexCount,
    InvalidVertex { index: usize },
    InvalidIndex { index: usize, vertex_count: usize },
    InvalidMaterial(MaterialEstimateError),
    TooLarge,
}

pub fn export_reconstruction_glb(
    reconstruction: &ReconstructionVolume,
    min_confidence: f32,
) -> Result<Vec<u8>, GlbExportError> {
    let mesh = reconstruction.extract_preview_mesh(min_confidence);
    export_preview_mesh_glb(&mesh)
}

pub fn export_reconstruction_glb_with_material(
    reconstruction: &ReconstructionVolume,
    min_confidence: f32,
    material: &PbrMaterialEstimate,
) -> Result<Vec<u8>, GlbExportError> {
    let mesh = reconstruction.extract_preview_mesh(min_confidence);
    export_preview_mesh_glb_with_material(&mesh, material)
}

pub fn export_preview_mesh_glb(mesh: &PreviewMesh) -> Result<Vec<u8>, GlbExportError> {
    export_preview_mesh_glb_internal(mesh, None)
}

pub fn export_preview_mesh_glb_with_material(
    mesh: &PreviewMesh,
    material: &PbrMaterialEstimate,
) -> Result<Vec<u8>, GlbExportError> {
    material
        .validate()
        .map_err(GlbExportError::InvalidMaterial)?;
    export_preview_mesh_glb_internal(mesh, Some(material))
}

fn export_preview_mesh_glb_internal(
    mesh: &PreviewMesh,
    material: Option<&PbrMaterialEstimate>,
) -> Result<Vec<u8>, GlbExportError> {
    validate_mesh(mesh)?;

    let vertex_count = mesh.vertices_session_meters.len();
    let index_count = mesh.triangle_indices.len();

    let mut binary = Vec::with_capacity(vertex_count * 12 + index_count * 4);
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];

    for (index, vertex) in mesh.vertices_session_meters.iter().enumerate() {
        let position = [
            vertex.x as f32,
            vertex.y as f32,
            vertex.z as f32,
        ];
        if !position.iter().all(|value| value.is_finite()) {
            return Err(GlbExportError::InvalidVertex { index });
        }

        for axis in 0..3 {
            min[axis] = min[axis].min(position[axis]);
            max[axis] = max[axis].max(position[axis]);
            binary.extend_from_slice(&position[axis].to_le_bytes());
        }
    }

    let positions_byte_length = binary.len();
    let indices_byte_offset = align_four(binary.len());
    binary.resize(indices_byte_offset, 0);

    for &index in &mesh.triangle_indices {
        binary.extend_from_slice(&index.to_le_bytes());
    }
    let indices_byte_length = binary.len() - indices_byte_offset;
    let binary_unpadded_length = binary.len();
    let binary_padded_length = align_four(binary_unpadded_length);
    binary.resize(binary_padded_length, 0);

    let material_reference = if material.is_some() {
        ",\"material\":0"
    } else {
        ""
    };
    let material_json = material.map_or(String::new(), |material| {
        format!(
            ",\"materials\":[{{\"pbrMetallicRoughness\":{{\"baseColorFactor\":[{},{},{},{}],\"metallicFactor\":{},\"roughnessFactor\":{}}}}}]",
            json_number(material.base_color_linear_rgba[0]),
            json_number(material.base_color_linear_rgba[1]),
            json_number(material.base_color_linear_rgba[2]),
            json_number(material.base_color_linear_rgba[3]),
            json_number(material.metallic),
            json_number(material.roughness),
        )
    });

    let json = format!(
        "{{\"asset\":{{\"version\":\"2.0\",\"generator\":\"xtreemze/scanner\"}},\"scene\":0,\"scenes\":[{{\"nodes\":[0]}}],\"nodes\":[{{\"mesh\":0}}],\"meshes\":[{{\"primitives\":[{{\"attributes\":{{\"POSITION\":0}},\"indices\":1,\"mode\":{TRIANGLES_MODE}{material_reference}}}]}}]{material_json},\"buffers\":[{{\"byteLength\":{binary_unpadded_length}}}],\"bufferViews\":[{{\"buffer\":0,\"byteOffset\":0,\"byteLength\":{positions_byte_length},\"target\":{ARRAY_BUFFER_TARGET}}},{{\"buffer\":0,\"byteOffset\":{indices_byte_offset},\"byteLength\":{indices_byte_length},\"target\":{ELEMENT_ARRAY_BUFFER_TARGET}}}],\"accessors\":[{{\"bufferView\":0,\"componentType\":{FLOAT_COMPONENT_TYPE},\"count\":{vertex_count},\"type\":\"VEC3\",\"min\":[{},{},{}],\"max\":[{},{},{}]}},{{\"bufferView\":1,\"componentType\":{UNSIGNED_INT_COMPONENT_TYPE},\"count\":{index_count},\"type\":\"SCALAR\"}}]}}",
        json_number(min[0]),
        json_number(min[1]),
        json_number(min[2]),
        json_number(max[0]),
        json_number(max[1]),
        json_number(max[2]),
    );

    let mut json_bytes = json.into_bytes();
    let json_padded_length = align_four(json_bytes.len());
    json_bytes.resize(json_padded_length, b' ');

    let total_length = 12usize
        .checked_add(8)
        .and_then(|v| v.checked_add(json_bytes.len()))
        .and_then(|v| v.checked_add(8))
        .and_then(|v| v.checked_add(binary.len()))
        .ok_or(GlbExportError::TooLarge)?;

    if total_length > u32::MAX as usize
        || json_bytes.len() > u32::MAX as usize
        || binary.len() > u32::MAX as usize
    {
        return Err(GlbExportError::TooLarge);
    }

    let mut glb = Vec::with_capacity(total_length);
    push_u32(&mut glb, GLB_MAGIC);
    push_u32(&mut glb, GLB_VERSION);
    push_u32(&mut glb, total_length as u32);

    push_u32(&mut glb, json_bytes.len() as u32);
    push_u32(&mut glb, JSON_CHUNK_TYPE);
    glb.extend_from_slice(&json_bytes);

    push_u32(&mut glb, binary.len() as u32);
    push_u32(&mut glb, BIN_CHUNK_TYPE);
    glb.extend_from_slice(&binary);

    Ok(glb)
}

fn validate_mesh(mesh: &PreviewMesh) -> Result<(), GlbExportError> {
    if mesh.vertices_session_meters.is_empty() || mesh.triangle_indices.is_empty() {
        return Err(GlbExportError::EmptyMesh);
    }
    if mesh.triangle_indices.len() % 3 != 0 {
        return Err(GlbExportError::NonTriangleIndexCount);
    }

    let vertex_count = mesh.vertices_session_meters.len();
    for (index, vertex) in mesh.vertices_session_meters.iter().enumerate() {
        if !vertex.x.is_finite() || !vertex.y.is_finite() || !vertex.z.is_finite() {
            return Err(GlbExportError::InvalidVertex { index });
        }
    }
    for &index in &mesh.triangle_indices {
        if index as usize >= vertex_count {
            return Err(GlbExportError::InvalidIndex {
                index: index as usize,
                vertex_count,
            });
        }
    }
    Ok(())
}

fn align_four(value: usize) -> usize {
    (value + 3) & !3
}

fn push_u32(target: &mut Vec<u8>, value: u32) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn json_number(value: f32) -> String {
    if value == 0.0 {
        "0".to_owned()
    } else {
        format!("{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        appearance::PbrMaterialEstimate,
        observation::Vec3,
    };

    fn triangle() -> PreviewMesh {
        PreviewMesh {
            vertices_session_meters: vec![
                Vec3 {
                    x: -1.0,
                    y: 0.0,
                    z: 0.5,
                },
                Vec3 {
                    x: 2.0,
                    y: 0.0,
                    z: 0.5,
                },
                Vec3 {
                    x: 0.0,
                    y: 3.0,
                    z: 0.5,
                },
            ],
            triangle_indices: vec![0, 1, 2],
        }
    }

    fn read_u32(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }

    #[test]
    fn emits_valid_glb_header_and_aligned_chunks() {
        let glb = export_preview_mesh_glb(&triangle()).unwrap();

        assert_eq!(read_u32(&glb, 0), GLB_MAGIC);
        assert_eq!(read_u32(&glb, 4), GLB_VERSION);
        assert_eq!(read_u32(&glb, 8) as usize, glb.len());

        let json_length = read_u32(&glb, 12) as usize;
        assert_eq!(json_length % 4, 0);
        assert_eq!(read_u32(&glb, 16), JSON_CHUNK_TYPE);

        let bin_header = 20 + json_length;
        let bin_length = read_u32(&glb, bin_header) as usize;
        assert_eq!(bin_length % 4, 0);
        assert_eq!(read_u32(&glb, bin_header + 4), BIN_CHUNK_TYPE);
        assert_eq!(bin_header + 8 + bin_length, glb.len());
    }

    #[test]
    fn json_describes_metric_positions_and_triangle_indices() {
        let glb = export_preview_mesh_glb(&triangle()).unwrap();
        let json_length = read_u32(&glb, 12) as usize;
        let json = std::str::from_utf8(&glb[20..20 + json_length])
            .unwrap()
            .trim_end();

        assert!(json.contains("\"asset\":{\"version\":\"2.0\""));
        assert!(json.contains("\"count\":3,\"type\":\"VEC3\""));
        assert!(json.contains("\"min\":[-1,0,0.5]"));
        assert!(json.contains("\"max\":[2,3,0.5]"));
        assert!(json.contains("\"componentType\":5125,\"count\":3,\"type\":\"SCALAR\""));
    }

    #[test]
    fn binary_chunk_contains_little_endian_f32_positions_and_u32_indices() {
        let glb = export_preview_mesh_glb(&triangle()).unwrap();
        let json_length = read_u32(&glb, 12) as usize;
        let bin_start = 20 + json_length + 8;

        assert_eq!(
            &glb[bin_start..bin_start + 4],
            &(-1.0f32).to_le_bytes()
        );

        let index_start = bin_start + 3 * 3 * 4;
        assert_eq!(&glb[index_start..index_start + 4], &0u32.to_le_bytes());
        assert_eq!(&glb[index_start + 4..index_start + 8], &1u32.to_le_bytes());
        assert_eq!(&glb[index_start + 8..index_start + 12], &2u32.to_le_bytes());
    }

    #[test]
    fn validated_pbr_material_is_projected_into_glb() {
        let material = PbrMaterialEstimate {
            base_color_linear_rgba: [0.2, 0.3, 0.4, 1.0],
            metallic: 0.25,
            roughness: 0.75,
            confidence: 0.8,
            evidence_ids: vec!["pair-a".into()],
        };
        let glb = export_preview_mesh_glb_with_material(&triangle(), &material).unwrap();
        let json_length = read_u32(&glb, 12) as usize;
        let json = std::str::from_utf8(&glb[20..20 + json_length])
            .unwrap()
            .trim_end();

        assert!(json.contains("\"material\":0"));
        assert!(json.contains("\"baseColorFactor\":[0.2,0.3,0.4,1]"));
        assert!(json.contains("\"metallicFactor\":0.25"));
        assert!(json.contains("\"roughnessFactor\":0.75"));
    }

    #[test]
    fn invalid_pbr_material_is_rejected_before_export() {
        let material = PbrMaterialEstimate {
            base_color_linear_rgba: [0.2, 0.3, 0.4, 1.0],
            metallic: 0.0,
            roughness: 2.0,
            confidence: 0.8,
            evidence_ids: vec!["pair-a".into()],
        };
        assert_eq!(
            export_preview_mesh_glb_with_material(&triangle(), &material),
            Err(GlbExportError::InvalidMaterial(
                MaterialEstimateError::InvalidRoughness
            ))
        );
    }

    #[test]
    fn rejects_out_of_range_indices() {
        let mut mesh = triangle();
        mesh.triangle_indices[2] = 3;
        assert_eq!(
            export_preview_mesh_glb(&mesh),
            Err(GlbExportError::InvalidIndex {
                index: 3,
                vertex_count: 3,
            })
        );
    }

    #[test]
    fn rejects_non_triangle_index_count() {
        let mut mesh = triangle();
        mesh.triangle_indices.pop();
        assert_eq!(
            export_preview_mesh_glb(&mesh),
            Err(GlbExportError::NonTriangleIndexCount)
        );
    }

    #[test]
    fn rejects_non_finite_vertices() {
        let mut mesh = triangle();
        mesh.vertices_session_meters[0].x = f64::NAN;
        assert_eq!(
            export_preview_mesh_glb(&mesh),
            Err(GlbExportError::InvalidVertex { index: 0 })
        );
    }
}
