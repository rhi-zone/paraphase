//! 3D format converters for Paraphase.
//!
//! Provides STL ↔ OBJ ↔ PLY ↔ glTF conversions via a shared triangle mesh IR.
//!
//! # Features
//! - `stl` (default) — STL binary meshes via `stl_io`
//! - `obj` — Wavefront OBJ via `tobj`
//! - `ply` — PLY point clouds/meshes via `ply-rs`
//! - `gltf` — glTF/GLB via `gltf`

use paraphase_core::{
    ConvertError, ConvertOutput, Converter, ConverterDecl, Properties, PropertyPattern, Registry,
};

/// Register all enabled 3D converters with the registry.
pub fn register_all(registry: &mut Registry) {
    #[cfg(feature = "stl")]
    {
        #[cfg(feature = "obj")]
        {
            registry.register(StlToObj);
            registry.register(ObjToStl);
        }
        #[cfg(feature = "ply")]
        {
            registry.register(StlToPly);
            registry.register(PlyToStl);
        }
        #[cfg(feature = "gltf")]
        {
            registry.register(StlToGltf);
            registry.register(GltfToStl);
        }
    }
    #[cfg(feature = "obj")]
    {
        #[cfg(feature = "ply")]
        {
            registry.register(ObjToPly);
            registry.register(PlyToObj);
        }
        #[cfg(feature = "gltf")]
        {
            registry.register(ObjToGltf);
            registry.register(GltfToObj);
        }
    }
    #[cfg(all(feature = "ply", feature = "gltf"))]
    {
        registry.register(PlyToGltf);
        registry.register(GltfToPly);
    }
}

// ============================================================
// Shared mesh IR
// ============================================================

struct Mesh {
    vertices: Vec<[f32; 3]>,
    faces: Vec<[u32; 3]>,
}

fn compute_normal(v0: [f32; 3], v1: [f32; 3], v2: [f32; 3]) -> [f32; 3] {
    let ax = v1[0] - v0[0];
    let ay = v1[1] - v0[1];
    let az = v1[2] - v0[2];
    let bx = v2[0] - v0[0];
    let by = v2[1] - v0[1];
    let bz = v2[2] - v0[2];
    let nx = ay * bz - az * by;
    let ny = az * bx - ax * bz;
    let nz = ax * by - ay * bx;
    let len = (nx * nx + ny * ny + nz * nz).sqrt();
    if len < 1e-10 {
        [0.0, 0.0, 1.0]
    } else {
        [nx / len, ny / len, nz / len]
    }
}

// ============================================================
// STL
// ============================================================

#[cfg(feature = "stl")]
fn stl_to_mesh(input: &[u8]) -> Result<Mesh, ConvertError> {
    use std::io::Cursor;
    let mut cursor = Cursor::new(input);
    let indexed = stl_io::read_stl(&mut cursor)
        .map_err(|e| ConvertError::InvalidInput(format!("Invalid STL: {e}")))?;
    let vertices: Vec<[f32; 3]> = indexed.vertices.iter().map(|v| v.0).collect();
    let faces: Vec<[u32; 3]> = indexed
        .faces
        .iter()
        .map(|f| {
            [
                f.vertices[0] as u32,
                f.vertices[1] as u32,
                f.vertices[2] as u32,
            ]
        })
        .collect();
    Ok(Mesh { vertices, faces })
}

#[cfg(feature = "stl")]
fn mesh_to_stl(mesh: &Mesh) -> Result<Vec<u8>, ConvertError> {
    let triangles: Vec<stl_io::Triangle> = mesh
        .faces
        .iter()
        .map(|face| {
            let v0 = mesh.vertices[face[0] as usize];
            let v1 = mesh.vertices[face[1] as usize];
            let v2 = mesh.vertices[face[2] as usize];
            stl_io::Triangle {
                normal: stl_io::Normal::new(compute_normal(v0, v1, v2)),
                vertices: [
                    stl_io::Vertex::new(v0),
                    stl_io::Vertex::new(v1),
                    stl_io::Vertex::new(v2),
                ],
            }
        })
        .collect();
    let mut output = Vec::new();
    stl_io::write_stl(&mut output, triangles.iter())
        .map_err(|e| ConvertError::Failed(format!("STL write failed: {e}")))?;
    Ok(output)
}

// ============================================================
// OBJ
// ============================================================

#[cfg(feature = "obj")]
fn obj_to_mesh(input: &[u8]) -> Result<Mesh, ConvertError> {
    use std::io::{BufReader, Cursor};
    let mut reader = BufReader::new(Cursor::new(input));
    let opts = tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ..Default::default()
    };
    let (models, _) =
        tobj::load_obj_buf(&mut reader, &opts, |_| Err(tobj::LoadError::GenericFailure))
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid OBJ: {e}")))?;

    let mut vertices: Vec<[f32; 3]> = Vec::new();
    let mut faces: Vec<[u32; 3]> = Vec::new();
    for model in &models {
        let m = &model.mesh;
        let vert_offset = vertices.len() as u32;
        let nv = m.positions.len() / 3;
        for i in 0..nv {
            vertices.push([
                m.positions[i * 3],
                m.positions[i * 3 + 1],
                m.positions[i * 3 + 2],
            ]);
        }
        let nf = m.indices.len() / 3;
        for i in 0..nf {
            faces.push([
                vert_offset + m.indices[i * 3],
                vert_offset + m.indices[i * 3 + 1],
                vert_offset + m.indices[i * 3 + 2],
            ]);
        }
    }
    Ok(Mesh { vertices, faces })
}

#[cfg(feature = "obj")]
fn mesh_to_obj(mesh: &Mesh) -> Vec<u8> {
    let mut out = String::new();
    for v in &mesh.vertices {
        out.push_str(&format!("v {} {} {}\n", v[0], v[1], v[2]));
    }
    for f in &mesh.faces {
        out.push_str(&format!("f {} {} {}\n", f[0] + 1, f[1] + 1, f[2] + 1));
    }
    out.into_bytes()
}

// ============================================================
// PLY
// ============================================================

#[cfg(feature = "ply")]
fn ply_to_mesh(input: &[u8]) -> Result<Mesh, ConvertError> {
    use ply_rs::parser::Parser;
    use ply_rs::ply::DefaultElement;
    use std::io::{BufReader, Cursor};

    let mut reader = BufReader::new(Cursor::new(input));
    let parser = Parser::<DefaultElement>::new();
    let ply = parser
        .read_ply(&mut reader)
        .map_err(|e| ConvertError::InvalidInput(format!("Invalid PLY: {e}")))?;

    let vertex_list = ply
        .payload
        .get("vertex")
        .ok_or_else(|| ConvertError::InvalidInput("PLY missing 'vertex' element".into()))?;

    let mut vertices = Vec::with_capacity(vertex_list.len());
    for elem in vertex_list {
        let x = ply_get_float(elem, "x")?;
        let y = ply_get_float(elem, "y")?;
        let z = ply_get_float(elem, "z")?;
        vertices.push([x, y, z]);
    }

    let mut faces = Vec::new();
    if let Some(face_list) = ply.payload.get("face") {
        for elem in face_list {
            let indices = ply_get_list(elem, "vertex_indices")
                .or_else(|_| ply_get_list(elem, "vertex_index"))?;
            // Fan triangulation for quads and higher-arity faces
            for i in 1..indices.len().saturating_sub(1) {
                faces.push([indices[0] as u32, indices[i] as u32, indices[i + 1] as u32]);
            }
        }
    }

    Ok(Mesh { vertices, faces })
}

#[cfg(feature = "ply")]
fn ply_get_float(elem: &ply_rs::ply::DefaultElement, key: &str) -> Result<f32, ConvertError> {
    use ply_rs::ply::Property;
    match elem.get(key) {
        Some(Property::Float(v)) => Ok(*v),
        Some(Property::Double(v)) => Ok(*v as f32),
        _ => Err(ConvertError::InvalidInput(format!(
            "PLY vertex missing '{key}' float property"
        ))),
    }
}

#[cfg(feature = "ply")]
fn ply_get_list(elem: &ply_rs::ply::DefaultElement, key: &str) -> Result<Vec<i64>, ConvertError> {
    use ply_rs::ply::Property;
    match elem.get(key) {
        Some(Property::ListInt(v)) => Ok(v.iter().map(|&x| x as i64).collect()),
        Some(Property::ListUInt(v)) => Ok(v.iter().map(|&x| x as i64).collect()),
        Some(Property::ListShort(v)) => Ok(v.iter().map(|&x| x as i64).collect()),
        Some(Property::ListUShort(v)) => Ok(v.iter().map(|&x| x as i64).collect()),
        Some(Property::ListChar(v)) => Ok(v.iter().map(|&x| x as i64).collect()),
        Some(Property::ListUChar(v)) => Ok(v.iter().map(|&x| x as i64).collect()),
        _ => Err(ConvertError::InvalidInput(format!(
            "PLY face missing '{key}' list property"
        ))),
    }
}

#[cfg(feature = "ply")]
fn mesh_to_ply(mesh: &Mesh) -> Vec<u8> {
    let mut out = String::new();
    out.push_str("ply\nformat ascii 1.0\ncomment Generated by paraphase-3d\n");
    out.push_str(&format!("element vertex {}\n", mesh.vertices.len()));
    out.push_str("property float x\nproperty float y\nproperty float z\n");
    out.push_str(&format!("element face {}\n", mesh.faces.len()));
    out.push_str("property list uchar int vertex_indices\nend_header\n");
    for v in &mesh.vertices {
        out.push_str(&format!("{} {} {}\n", v[0], v[1], v[2]));
    }
    for f in &mesh.faces {
        out.push_str(&format!("3 {} {} {}\n", f[0], f[1], f[2]));
    }
    out.into_bytes()
}

// ============================================================
// glTF / GLB
// ============================================================

#[cfg(feature = "gltf")]
fn gltf_to_mesh(input: &[u8]) -> Result<Mesh, ConvertError> {
    let (document, buffers, _) = gltf::import_slice(input)
        .map_err(|e| ConvertError::InvalidInput(format!("Invalid glTF/GLB: {e}")))?;

    let mut vertices: Vec<[f32; 3]> = Vec::new();
    let mut faces: Vec<[u32; 3]> = Vec::new();

    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            let vert_offset = vertices.len() as u32;
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
            let Some(pos_iter) = reader.read_positions() else {
                continue;
            };
            let prim_verts: Vec<[f32; 3]> = pos_iter.collect();
            let prim_count = prim_verts.len() as u32;
            vertices.extend(prim_verts);

            if let Some(idx_reader) = reader.read_indices() {
                let indices: Vec<u32> = idx_reader.into_u32().collect();
                for chunk in indices.chunks_exact(3) {
                    faces.push([
                        vert_offset + chunk[0],
                        vert_offset + chunk[1],
                        vert_offset + chunk[2],
                    ]);
                }
            } else {
                // Non-indexed: generate sequential triangle indices
                for i in (0..prim_count).step_by(3) {
                    if i + 2 < prim_count {
                        faces.push([vert_offset + i, vert_offset + i + 1, vert_offset + i + 2]);
                    }
                }
            }
        }
    }

    Ok(Mesh { vertices, faces })
}

#[cfg(feature = "gltf")]
fn mesh_to_glb(mesh: &Mesh) -> Vec<u8> {
    // Vertex buffer: f32 x3 per vertex, little-endian
    let mut vertex_buf: Vec<u8> = Vec::with_capacity(mesh.vertices.len() * 12);
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for v in &mesh.vertices {
        for (i, &c) in v.iter().enumerate() {
            if c < min[i] {
                min[i] = c;
            }
            if c > max[i] {
                max[i] = c;
            }
        }
        for &c in v.iter() {
            vertex_buf.extend_from_slice(&c.to_le_bytes());
        }
    }
    if mesh.vertices.is_empty() {
        min = [0.0; 3];
        max = [0.0; 3];
    }

    // Index buffer: u32 per index, little-endian
    let mut index_buf: Vec<u8> = Vec::with_capacity(mesh.faces.len() * 12);
    for f in &mesh.faces {
        for &idx in f.iter() {
            index_buf.extend_from_slice(&idx.to_le_bytes());
        }
    }

    // Pad to 4-byte alignment
    while !vertex_buf.len().is_multiple_of(4) {
        vertex_buf.push(0);
    }
    while !index_buf.len().is_multiple_of(4) {
        index_buf.push(0);
    }

    let vertex_byte_len = vertex_buf.len();
    let index_byte_len = index_buf.len();
    let buffer_byte_len = vertex_byte_len + index_byte_len;

    let json = format!(
        r#"{{"asset":{{"version":"2.0"}},"buffers":[{{"byteLength":{buffer_byte_len}}}],"bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":{vertex_byte_len},"target":34962}},{{"buffer":0,"byteOffset":{vertex_byte_len},"byteLength":{index_byte_len},"target":34963}}],"accessors":[{{"bufferView":0,"componentType":5126,"count":{nv},"type":"VEC3","min":[{minx},{miny},{minz}],"max":[{maxx},{maxy},{maxz}]}},{{"bufferView":1,"componentType":5125,"count":{ni},"type":"SCALAR"}}],"meshes":[{{"name":"mesh","primitives":[{{"attributes":{{"POSITION":0}},"indices":1}}]}}],"nodes":[{{"mesh":0}}],"scenes":[{{"nodes":[0]}}],"scene":0}}"#,
        nv = mesh.vertices.len(),
        ni = mesh.faces.len() * 3,
        minx = min[0],
        miny = min[1],
        minz = min[2],
        maxx = max[0],
        maxy = max[1],
        maxz = max[2],
    );

    let mut json_bytes = json.into_bytes();
    while !json_bytes.len().is_multiple_of(4) {
        json_bytes.push(b' ');
    }

    let json_chunk_len = json_bytes.len() as u32;
    let bin_chunk_len = (vertex_byte_len + index_byte_len) as u32;
    let total = 12u32 + 8 + json_chunk_len + 8 + bin_chunk_len;

    let mut out = Vec::with_capacity(total as usize);
    // GLB header
    out.extend_from_slice(&0x46546C67u32.to_le_bytes()); // magic "glTF"
    out.extend_from_slice(&2u32.to_le_bytes()); // version 2
    out.extend_from_slice(&total.to_le_bytes());
    // JSON chunk
    out.extend_from_slice(&json_chunk_len.to_le_bytes());
    out.extend_from_slice(&0x4E4F534Au32.to_le_bytes()); // "JSON"
    out.extend_from_slice(&json_bytes);
    // BIN chunk
    out.extend_from_slice(&bin_chunk_len.to_le_bytes());
    out.extend_from_slice(&0x004E4942u32.to_le_bytes()); // "BIN\0"
    out.extend_from_slice(&vertex_buf);
    out.extend_from_slice(&index_buf);
    out
}

// ============================================================
// Converter structs — STL ↔ OBJ
// ============================================================

#[cfg(all(feature = "stl", feature = "obj"))]
pub struct StlToObj;

#[cfg(all(feature = "stl", feature = "obj"))]
impl Converter for StlToObj {
    fn decl(&self) -> &ConverterDecl {
        static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
        DECL.get_or_init(|| {
            ConverterDecl::simple(
                "3d.stl-to-obj",
                PropertyPattern::new().eq("format", "stl"),
                PropertyPattern::new().eq("format", "obj"),
            )
            .description("Convert STL triangle mesh to Wavefront OBJ")
        })
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        let mesh = stl_to_mesh(input)?;
        let output = mesh_to_obj(&mesh);
        let mut out_props = props.clone();
        out_props.insert("format".into(), "obj".into());
        Ok(ConvertOutput::Single(output, out_props))
    }
}

#[cfg(all(feature = "stl", feature = "obj"))]
pub struct ObjToStl;

#[cfg(all(feature = "stl", feature = "obj"))]
impl Converter for ObjToStl {
    fn decl(&self) -> &ConverterDecl {
        static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
        DECL.get_or_init(|| {
            ConverterDecl::simple(
                "3d.obj-to-stl",
                PropertyPattern::new().eq("format", "obj"),
                PropertyPattern::new().eq("format", "stl"),
            )
            .description("Convert Wavefront OBJ to STL binary triangle mesh")
        })
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        let mesh = obj_to_mesh(input)?;
        let output = mesh_to_stl(&mesh)?;
        let mut out_props = props.clone();
        out_props.insert("format".into(), "stl".into());
        Ok(ConvertOutput::Single(output, out_props))
    }
}

// ============================================================
// Converter structs — STL ↔ PLY
// ============================================================

#[cfg(all(feature = "stl", feature = "ply"))]
pub struct StlToPly;

#[cfg(all(feature = "stl", feature = "ply"))]
impl Converter for StlToPly {
    fn decl(&self) -> &ConverterDecl {
        static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
        DECL.get_or_init(|| {
            ConverterDecl::simple(
                "3d.stl-to-ply",
                PropertyPattern::new().eq("format", "stl"),
                PropertyPattern::new().eq("format", "ply"),
            )
            .description("Convert STL triangle mesh to PLY")
        })
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        let mesh = stl_to_mesh(input)?;
        let output = mesh_to_ply(&mesh);
        let mut out_props = props.clone();
        out_props.insert("format".into(), "ply".into());
        Ok(ConvertOutput::Single(output, out_props))
    }
}

#[cfg(all(feature = "stl", feature = "ply"))]
pub struct PlyToStl;

#[cfg(all(feature = "stl", feature = "ply"))]
impl Converter for PlyToStl {
    fn decl(&self) -> &ConverterDecl {
        static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
        DECL.get_or_init(|| {
            ConverterDecl::simple(
                "3d.ply-to-stl",
                PropertyPattern::new().eq("format", "ply"),
                PropertyPattern::new().eq("format", "stl"),
            )
            .description("Convert PLY mesh to STL binary triangle mesh")
        })
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        let mesh = ply_to_mesh(input)?;
        let output = mesh_to_stl(&mesh)?;
        let mut out_props = props.clone();
        out_props.insert("format".into(), "stl".into());
        Ok(ConvertOutput::Single(output, out_props))
    }
}

// ============================================================
// Converter structs — STL ↔ glTF
// ============================================================

#[cfg(all(feature = "stl", feature = "gltf"))]
pub struct StlToGltf;

#[cfg(all(feature = "stl", feature = "gltf"))]
impl Converter for StlToGltf {
    fn decl(&self) -> &ConverterDecl {
        static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
        DECL.get_or_init(|| {
            ConverterDecl::simple(
                "3d.stl-to-glb",
                PropertyPattern::new().eq("format", "stl"),
                PropertyPattern::new().eq("format", "glb"),
            )
            .description("Convert STL triangle mesh to binary glTF (GLB)")
        })
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        let mesh = stl_to_mesh(input)?;
        let output = mesh_to_glb(&mesh);
        let mut out_props = props.clone();
        out_props.insert("format".into(), "glb".into());
        Ok(ConvertOutput::Single(output, out_props))
    }
}

#[cfg(all(feature = "stl", feature = "gltf"))]
pub struct GltfToStl;

#[cfg(all(feature = "stl", feature = "gltf"))]
impl Converter for GltfToStl {
    fn decl(&self) -> &ConverterDecl {
        static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
        DECL.get_or_init(|| {
            use paraphase_core::{Predicate, Value as PValue};
            ConverterDecl::simple(
                "3d.gltf-to-stl",
                PropertyPattern::new().with(
                    "format",
                    Predicate::OneOf(vec![PValue::from("gltf"), PValue::from("glb")]),
                ),
                PropertyPattern::new().eq("format", "stl"),
            )
            .description("Convert glTF/GLB to STL binary triangle mesh")
        })
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        let mesh = gltf_to_mesh(input)?;
        let output = mesh_to_stl(&mesh)?;
        let mut out_props = props.clone();
        out_props.insert("format".into(), "stl".into());
        Ok(ConvertOutput::Single(output, out_props))
    }
}

// ============================================================
// Converter structs — OBJ ↔ PLY
// ============================================================

#[cfg(all(feature = "obj", feature = "ply"))]
pub struct ObjToPly;

#[cfg(all(feature = "obj", feature = "ply"))]
impl Converter for ObjToPly {
    fn decl(&self) -> &ConverterDecl {
        static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
        DECL.get_or_init(|| {
            ConverterDecl::simple(
                "3d.obj-to-ply",
                PropertyPattern::new().eq("format", "obj"),
                PropertyPattern::new().eq("format", "ply"),
            )
            .description("Convert Wavefront OBJ to PLY")
        })
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        let mesh = obj_to_mesh(input)?;
        let output = mesh_to_ply(&mesh);
        let mut out_props = props.clone();
        out_props.insert("format".into(), "ply".into());
        Ok(ConvertOutput::Single(output, out_props))
    }
}

#[cfg(all(feature = "obj", feature = "ply"))]
pub struct PlyToObj;

#[cfg(all(feature = "obj", feature = "ply"))]
impl Converter for PlyToObj {
    fn decl(&self) -> &ConverterDecl {
        static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
        DECL.get_or_init(|| {
            ConverterDecl::simple(
                "3d.ply-to-obj",
                PropertyPattern::new().eq("format", "ply"),
                PropertyPattern::new().eq("format", "obj"),
            )
            .description("Convert PLY mesh to Wavefront OBJ")
        })
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        let mesh = ply_to_mesh(input)?;
        let output = mesh_to_obj(&mesh);
        let mut out_props = props.clone();
        out_props.insert("format".into(), "obj".into());
        Ok(ConvertOutput::Single(output, out_props))
    }
}

// ============================================================
// Converter structs — OBJ ↔ glTF
// ============================================================

#[cfg(all(feature = "obj", feature = "gltf"))]
pub struct ObjToGltf;

#[cfg(all(feature = "obj", feature = "gltf"))]
impl Converter for ObjToGltf {
    fn decl(&self) -> &ConverterDecl {
        static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
        DECL.get_or_init(|| {
            ConverterDecl::simple(
                "3d.obj-to-glb",
                PropertyPattern::new().eq("format", "obj"),
                PropertyPattern::new().eq("format", "glb"),
            )
            .description("Convert Wavefront OBJ to binary glTF (GLB)")
        })
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        let mesh = obj_to_mesh(input)?;
        let output = mesh_to_glb(&mesh);
        let mut out_props = props.clone();
        out_props.insert("format".into(), "glb".into());
        Ok(ConvertOutput::Single(output, out_props))
    }
}

#[cfg(all(feature = "obj", feature = "gltf"))]
pub struct GltfToObj;

#[cfg(all(feature = "obj", feature = "gltf"))]
impl Converter for GltfToObj {
    fn decl(&self) -> &ConverterDecl {
        static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
        DECL.get_or_init(|| {
            use paraphase_core::{Predicate, Value as PValue};
            ConverterDecl::simple(
                "3d.gltf-to-obj",
                PropertyPattern::new().with(
                    "format",
                    Predicate::OneOf(vec![PValue::from("gltf"), PValue::from("glb")]),
                ),
                PropertyPattern::new().eq("format", "obj"),
            )
            .description("Convert glTF/GLB to Wavefront OBJ")
        })
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        let mesh = gltf_to_mesh(input)?;
        let output = mesh_to_obj(&mesh);
        let mut out_props = props.clone();
        out_props.insert("format".into(), "obj".into());
        Ok(ConvertOutput::Single(output, out_props))
    }
}

// ============================================================
// Converter structs — PLY ↔ glTF
// ============================================================

#[cfg(all(feature = "ply", feature = "gltf"))]
pub struct PlyToGltf;

#[cfg(all(feature = "ply", feature = "gltf"))]
impl Converter for PlyToGltf {
    fn decl(&self) -> &ConverterDecl {
        static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
        DECL.get_or_init(|| {
            ConverterDecl::simple(
                "3d.ply-to-glb",
                PropertyPattern::new().eq("format", "ply"),
                PropertyPattern::new().eq("format", "glb"),
            )
            .description("Convert PLY mesh to binary glTF (GLB)")
        })
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        let mesh = ply_to_mesh(input)?;
        let output = mesh_to_glb(&mesh);
        let mut out_props = props.clone();
        out_props.insert("format".into(), "glb".into());
        Ok(ConvertOutput::Single(output, out_props))
    }
}

#[cfg(all(feature = "ply", feature = "gltf"))]
pub struct GltfToPly;

#[cfg(all(feature = "ply", feature = "gltf"))]
impl Converter for GltfToPly {
    fn decl(&self) -> &ConverterDecl {
        static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
        DECL.get_or_init(|| {
            use paraphase_core::{Predicate, Value as PValue};
            ConverterDecl::simple(
                "3d.gltf-to-ply",
                PropertyPattern::new().with(
                    "format",
                    Predicate::OneOf(vec![PValue::from("gltf"), PValue::from("glb")]),
                ),
                PropertyPattern::new().eq("format", "ply"),
            )
            .description("Convert glTF/GLB to PLY mesh")
        })
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        let mesh = gltf_to_mesh(input)?;
        let output = mesh_to_ply(&mesh);
        let mut out_props = props.clone();
        out_props.insert("format".into(), "ply".into());
        Ok(ConvertOutput::Single(output, out_props))
    }
}
