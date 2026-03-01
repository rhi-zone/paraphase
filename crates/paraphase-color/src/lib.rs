//! Color palette format converters for Paraphase — pure Rust.
//!
//! Implements GPL (GIMP), ACO (Photoshop), and ASE (Adobe) palette formats.
//!
//! # Features
//! - `gpl` (default) — GIMP Palette format
//! - `aco` (default) — Photoshop Color Swatches
//! - `ase` (default) — Adobe Swatch Exchange

use paraphase_core::{
    ConvertError, ConvertOutput, Converter, ConverterDecl, Properties, PropertyPattern, Registry,
};
use serde_json::{Value, json};

/// Register all enabled color converters with the registry.
pub fn register_all(registry: &mut Registry) {
    #[cfg(feature = "gpl")]
    {
        registry.register(GplToJson);
        registry.register(JsonToGpl);
    }
    #[cfg(feature = "aco")]
    {
        registry.register(AcoToJson);
        registry.register(JsonToAco);
    }
    #[cfg(feature = "ase")]
    {
        registry.register(AseToJson);
        registry.register(JsonToAse);
    }
}

// ============================================
// Shared data model
// ============================================

/// A single color with optional name.
#[derive(Debug, Clone)]
pub struct Color {
    pub name: Option<String>,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// A palette of colors with optional name.
#[derive(Debug, Clone, Default)]
pub struct Palette {
    pub name: Option<String>,
    pub colors: Vec<Color>,
}

impl Palette {
    fn to_json(&self) -> Value {
        let colors: Vec<Value> = self
            .colors
            .iter()
            .map(|c| {
                let mut obj = serde_json::Map::new();
                obj.insert("r".into(), json!(c.r));
                obj.insert("g".into(), json!(c.g));
                obj.insert("b".into(), json!(c.b));
                if let Some(ref name) = c.name {
                    obj.insert("name".into(), json!(name));
                }
                Value::Object(obj)
            })
            .collect();

        let mut obj = serde_json::Map::new();
        if let Some(ref name) = self.name {
            obj.insert("name".into(), json!(name));
        }
        obj.insert("colors".into(), Value::Array(colors));
        Value::Object(obj)
    }

    fn from_json(value: &Value) -> Result<Self, ConvertError> {
        let colors_arr = value
            .get("colors")
            .and_then(|c| c.as_array())
            .ok_or_else(|| ConvertError::InvalidInput("Expected 'colors' array in JSON".into()))?;

        let colors: Result<Vec<Color>, ConvertError> = colors_arr
            .iter()
            .map(|c| {
                let r = c
                    .get("r")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u8)
                    .ok_or_else(|| ConvertError::InvalidInput("Color missing 'r' field".into()))?;
                let g = c
                    .get("g")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u8)
                    .ok_or_else(|| ConvertError::InvalidInput("Color missing 'g' field".into()))?;
                let b = c
                    .get("b")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u8)
                    .ok_or_else(|| ConvertError::InvalidInput("Color missing 'b' field".into()))?;
                let name = c
                    .get("name")
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string());
                Ok(Color { r, g, b, name })
            })
            .collect();

        Ok(Palette {
            name: value
                .get("name")
                .and_then(|n| n.as_str())
                .map(|s| s.to_string()),
            colors: colors?,
        })
    }
}

// ============================================
// GPL — GIMP Palette
// ============================================

#[cfg(feature = "gpl")]
mod gpl_impl {
    use super::*;

    /// Parse a GIMP Palette (.gpl) file to JSON.
    pub struct GplToJson;

    impl Converter for GplToJson {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "color.gpl-to-json",
                    PropertyPattern::new().eq("format", "gpl"),
                    PropertyPattern::new().eq("format", "json"),
                )
                .description("Parse GIMP Palette (.gpl) to JSON color palette")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let text = std::str::from_utf8(input)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?;

            let palette = parse_gpl(text)?;
            let json = palette.to_json();
            let output = serde_json::to_vec_pretty(&json)
                .map_err(|e| ConvertError::Failed(format!("JSON serialization failed: {}", e)))?;

            let mut out_props = props.clone();
            out_props.insert("format".into(), "json".into());
            Ok(ConvertOutput::Single(output, out_props))
        }
    }

    /// Serialize a JSON color palette to GIMP Palette (.gpl) format.
    pub struct JsonToGpl;

    impl Converter for JsonToGpl {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "color.json-to-gpl",
                    PropertyPattern::new().eq("format", "json"),
                    PropertyPattern::new().eq("format", "gpl"),
                )
                .description("Serialize JSON color palette to GIMP Palette (.gpl)")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let value: Value = serde_json::from_slice(input)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid JSON: {}", e)))?;
            let palette = Palette::from_json(&value)?;
            let gpl = serialize_gpl(&palette);

            let mut out_props = props.clone();
            out_props.insert("format".into(), "gpl".into());
            Ok(ConvertOutput::Single(gpl.into_bytes(), out_props))
        }
    }

    pub fn parse_gpl(text: &str) -> Result<Palette, ConvertError> {
        let mut lines = text.lines();
        let first = lines.next().unwrap_or("").trim();
        if first != "GIMP Palette" {
            return Err(ConvertError::InvalidInput(
                "Not a GIMP Palette file (missing 'GIMP Palette' header)".into(),
            ));
        }

        let mut palette = Palette::default();
        let mut in_colors = false;

        for line in lines {
            let line = line.trim();

            if let Some(rest) = line.strip_prefix("Name:") {
                palette.name = Some(rest.trim().to_string());
                continue;
            }
            if line.starts_with("Columns:") || line.starts_with('#') {
                if line == "#" {
                    in_colors = true;
                }
                continue;
            }
            if line.is_empty() {
                continue;
            }

            // Color line: "R G B [Name]" (whitespace-separated, R/G/B can be space-padded)
            let mut tokens = line.split_whitespace();
            if let (Some(rp), Some(gp), Some(bp)) = (tokens.next(), tokens.next(), tokens.next())
                && let (Ok(r), Ok(g), Ok(b)) =
                    (rp.parse::<u8>(), gp.parse::<u8>(), bp.parse::<u8>())
            {
                // Rest of tokens form the color name
                let remaining: Vec<&str> = tokens.collect();
                let name = if remaining.is_empty() {
                    None
                } else {
                    Some(remaining.join(" "))
                };
                palette.colors.push(Color { r, g, b, name });
                in_colors = true;
            }
        }

        if !in_colors && palette.colors.is_empty() {
            // Try parsing without the '#' separator (some tools omit it)
        }

        Ok(palette)
    }

    pub fn serialize_gpl(palette: &Palette) -> String {
        let mut out = String::from("GIMP Palette\n");
        if let Some(ref name) = palette.name {
            out.push_str(&format!("Name: {}\n", name));
        }
        out.push_str("Columns: 16\n#\n");
        for color in &palette.colors {
            let name_part = color.name.as_deref().unwrap_or("Untitled");
            out.push_str(&format!(
                "{:3} {:3} {:3}  {}\n",
                color.r, color.g, color.b, name_part
            ));
        }
        out
    }
}

#[cfg(feature = "gpl")]
pub use gpl_impl::{GplToJson, JsonToGpl};

// ============================================
// ACO — Photoshop Color Swatches
// ============================================

#[cfg(feature = "aco")]
mod aco_impl {
    use super::*;

    const COLORSPACE_RGB: u16 = 0;
    const COLORSPACE_GRAYSCALE: u16 = 8;

    fn read_u16_be(data: &[u8], offset: usize) -> Option<u16> {
        if offset + 2 > data.len() {
            return None;
        }
        Some(u16::from_be_bytes([data[offset], data[offset + 1]]))
    }

    fn write_u16_be(buf: &mut Vec<u8>, v: u16) {
        buf.extend_from_slice(&v.to_be_bytes());
    }

    /// Parse Photoshop Color Swatches (.aco) to JSON.
    pub struct AcoToJson;

    impl Converter for AcoToJson {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "color.aco-to-json",
                    PropertyPattern::new().eq("format", "aco"),
                    PropertyPattern::new().eq("format", "json"),
                )
                .description("Parse Photoshop Color Swatches (.aco) to JSON color palette")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let palette = parse_aco(input)?;
            let json = palette.to_json();
            let output = serde_json::to_vec_pretty(&json)
                .map_err(|e| ConvertError::Failed(format!("JSON serialization failed: {}", e)))?;

            let mut out_props = props.clone();
            out_props.insert("format".into(), "json".into());
            Ok(ConvertOutput::Single(output, out_props))
        }
    }

    /// Serialize JSON color palette to Photoshop Color Swatches (.aco) format.
    pub struct JsonToAco;

    impl Converter for JsonToAco {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "color.json-to-aco",
                    PropertyPattern::new().eq("format", "json"),
                    PropertyPattern::new().eq("format", "aco"),
                )
                .description("Serialize JSON color palette to Photoshop Color Swatches (.aco)")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let value: Value = serde_json::from_slice(input)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid JSON: {}", e)))?;
            let palette = Palette::from_json(&value)?;
            let aco = serialize_aco(&palette);

            let mut out_props = props.clone();
            out_props.insert("format".into(), "aco".into());
            Ok(ConvertOutput::Single(aco, out_props))
        }
    }

    fn aco_component_to_u8(high: u16) -> u8 {
        // ACO stores values as 0..=65535; for RGB, it's 0..=65535 mapping to 0..=255
        (high >> 8) as u8
    }

    fn u8_to_aco_component(v: u8) -> u16 {
        (v as u16) << 8
    }

    pub fn parse_aco(data: &[u8]) -> Result<Palette, ConvertError> {
        if data.len() < 4 {
            return Err(ConvertError::InvalidInput("ACO file too short".into()));
        }

        let _version = read_u16_be(data, 0)
            .ok_or_else(|| ConvertError::InvalidInput("Cannot read ACO version".into()))?;
        let count = read_u16_be(data, 2)
            .ok_or_else(|| ConvertError::InvalidInput("Cannot read ACO color count".into()))?;

        // Try to find v2 block (comes after v1 block)
        let v1_size = 4 + count as usize * 10; // header(4) + colors(10 each)

        // Check if we have a v2 block
        let v2_offset = if data.len() > v1_size + 4 {
            let v2_ver = read_u16_be(data, v1_size);
            let v2_cnt = read_u16_be(data, v1_size + 2);
            if v2_ver == Some(2) && v2_cnt == Some(count) {
                Some(v1_size)
            } else {
                None
            }
        } else {
            None
        };

        let mut colors = Vec::with_capacity(count as usize);

        for i in 0..count as usize {
            let base = 4 + i * 10;
            if base + 10 > data.len() {
                break;
            }

            let colorspace = read_u16_be(data, base).unwrap_or(0);
            let c1 = read_u16_be(data, base + 2).unwrap_or(0);
            let c2 = read_u16_be(data, base + 4).unwrap_or(0);
            let c3 = read_u16_be(data, base + 6).unwrap_or(0);
            // c4 at base+8 for CMYK

            let (r, g, b) = match colorspace {
                COLORSPACE_RGB => (
                    aco_component_to_u8(c1),
                    aco_component_to_u8(c2),
                    aco_component_to_u8(c3),
                ),
                COLORSPACE_GRAYSCALE => {
                    let v = aco_component_to_u8(c1);
                    (v, v, v)
                }
                _ => {
                    // Convert other colorspaces to RGB approximately
                    (
                        aco_component_to_u8(c1),
                        aco_component_to_u8(c2),
                        aco_component_to_u8(c3),
                    )
                }
            };

            // Read name from v2 block if available
            let name = v2_offset.and_then(|v2_base| {
                // v2 block: header(4) + for each color: 10 bytes + 2 bytes null + pascal-style UTF-16BE name
                // We need to iterate to find this color's name
                let mut pos = v2_base + 4; // skip v2 header
                for j in 0..=i {
                    if pos + 10 > data.len() {
                        return None;
                    }
                    pos += 10; // skip colorspace + 4 components
                    if pos + 2 > data.len() {
                        return None;
                    }
                    pos += 2; // skip padding (0x0000)
                    // Read name length (uint16 = number of UTF-16 code units, including null terminator)
                    let name_len = read_u16_be(data, pos)? as usize;
                    pos += 2;
                    let name_bytes = name_len * 2; // UTF-16BE
                    if pos + name_bytes > data.len() {
                        return None;
                    }
                    if j == i {
                        // Decode UTF-16BE, strip null terminator
                        let utf16: Vec<u16> = (0..name_len)
                            .map(|k| u16::from_be_bytes([data[pos + k * 2], data[pos + k * 2 + 1]]))
                            .filter(|&c| c != 0)
                            .collect();
                        return String::from_utf16(&utf16).ok();
                    }
                    pos += name_bytes;
                }
                None
            });

            colors.push(Color { r, g, b, name });
        }

        Ok(Palette { name: None, colors })
    }

    pub fn serialize_aco(palette: &Palette) -> Vec<u8> {
        let count = palette.colors.len() as u16;
        let mut buf = Vec::new();

        // Version 1 block
        write_u16_be(&mut buf, 1);
        write_u16_be(&mut buf, count);
        for color in &palette.colors {
            write_u16_be(&mut buf, COLORSPACE_RGB);
            write_u16_be(&mut buf, u8_to_aco_component(color.r));
            write_u16_be(&mut buf, u8_to_aco_component(color.g));
            write_u16_be(&mut buf, u8_to_aco_component(color.b));
            write_u16_be(&mut buf, 0); // component 4 (unused for RGB)
        }

        // Version 2 block (includes names)
        write_u16_be(&mut buf, 2);
        write_u16_be(&mut buf, count);
        for color in &palette.colors {
            write_u16_be(&mut buf, COLORSPACE_RGB);
            write_u16_be(&mut buf, u8_to_aco_component(color.r));
            write_u16_be(&mut buf, u8_to_aco_component(color.g));
            write_u16_be(&mut buf, u8_to_aco_component(color.b));
            write_u16_be(&mut buf, 0); // component 4 (unused for RGB)
            write_u16_be(&mut buf, 0); // padding
            // Name as UTF-16BE with null terminator
            let name = color.name.as_deref().unwrap_or("Untitled");
            let utf16: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            write_u16_be(&mut buf, utf16.len() as u16);
            for code_unit in &utf16 {
                write_u16_be(&mut buf, *code_unit);
            }
        }

        buf
    }
}

#[cfg(feature = "aco")]
pub use aco_impl::{AcoToJson, JsonToAco};

// ============================================
// ASE — Adobe Swatch Exchange
// ============================================

#[cfg(feature = "ase")]
mod ase_impl {
    use super::*;

    const ASE_MAGIC: &[u8; 4] = b"ASEF";
    const BLOCK_COLOR: u16 = 0x0001;
    const BLOCK_GROUP_START: u16 = 0xC001;
    const BLOCK_GROUP_END: u16 = 0xC002;

    fn read_u16_be(data: &[u8], offset: usize) -> Option<u16> {
        if offset + 2 > data.len() {
            return None;
        }
        Some(u16::from_be_bytes([data[offset], data[offset + 1]]))
    }

    fn read_u32_be(data: &[u8], offset: usize) -> Option<u32> {
        if offset + 4 > data.len() {
            return None;
        }
        Some(u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]))
    }

    fn write_u16_be(buf: &mut Vec<u8>, v: u16) {
        buf.extend_from_slice(&v.to_be_bytes());
    }

    fn write_u32_be(buf: &mut Vec<u8>, v: u32) {
        buf.extend_from_slice(&v.to_be_bytes());
    }

    fn write_f32_be(buf: &mut Vec<u8>, v: f32) {
        buf.extend_from_slice(&v.to_be_bytes());
    }

    /// Parse Adobe Swatch Exchange (.ase) to JSON.
    pub struct AseToJson;

    impl Converter for AseToJson {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "color.ase-to-json",
                    PropertyPattern::new().eq("format", "ase"),
                    PropertyPattern::new().eq("format", "json"),
                )
                .description("Parse Adobe Swatch Exchange (.ase) to JSON color palette")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let palette = parse_ase(input)?;
            let json = palette.to_json();
            let output = serde_json::to_vec_pretty(&json)
                .map_err(|e| ConvertError::Failed(format!("JSON serialization failed: {}", e)))?;

            let mut out_props = props.clone();
            out_props.insert("format".into(), "json".into());
            Ok(ConvertOutput::Single(output, out_props))
        }
    }

    /// Serialize JSON color palette to Adobe Swatch Exchange (.ase) format.
    pub struct JsonToAse;

    impl Converter for JsonToAse {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "color.json-to-ase",
                    PropertyPattern::new().eq("format", "json"),
                    PropertyPattern::new().eq("format", "ase"),
                )
                .description("Serialize JSON color palette to Adobe Swatch Exchange (.ase)")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let value: Value = serde_json::from_slice(input)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid JSON: {}", e)))?;
            let palette = Palette::from_json(&value)?;
            let ase = serialize_ase(&palette);

            let mut out_props = props.clone();
            out_props.insert("format".into(), "ase".into());
            Ok(ConvertOutput::Single(ase, out_props))
        }
    }

    pub fn parse_ase(data: &[u8]) -> Result<Palette, ConvertError> {
        if data.len() < 12 {
            return Err(ConvertError::InvalidInput("ASE file too short".into()));
        }
        if &data[0..4] != ASE_MAGIC {
            return Err(ConvertError::InvalidInput(
                "Not an ASE file (bad magic)".into(),
            ));
        }

        // Skip version (bytes 4-7) and block count (bytes 8-11)
        let _version = read_u32_be(data, 4);
        let block_count = read_u32_be(data, 8).unwrap_or(0);

        let mut pos = 12;
        let mut colors = Vec::new();

        for _ in 0..block_count {
            if pos + 6 > data.len() {
                break;
            }
            let block_type = read_u16_be(data, pos).unwrap_or(0);
            let block_len = read_u32_be(data, pos + 2).unwrap_or(0) as usize;
            pos += 6;

            let block_end = pos + block_len;

            match block_type {
                BLOCK_COLOR => {
                    if pos + 2 > data.len() {
                        break;
                    }
                    // Name: uint16 length + UTF-16BE chars (includes null terminator)
                    let name_len = read_u16_be(data, pos).unwrap_or(0) as usize;
                    pos += 2;
                    let name_bytes = name_len * 2;
                    let name = if pos + name_bytes <= data.len() && name_len > 0 {
                        let utf16: Vec<u16> = (0..name_len)
                            .map(|k| u16::from_be_bytes([data[pos + k * 2], data[pos + k * 2 + 1]]))
                            .filter(|&c| c != 0)
                            .collect();
                        String::from_utf16(&utf16).ok()
                    } else {
                        None
                    };
                    pos += name_bytes;

                    // Color model: 4 bytes
                    if pos + 4 > data.len() {
                        pos = block_end;
                        continue;
                    }
                    let model = &data[pos..pos + 4];
                    pos += 4;

                    let (r, g, b) = if model == b"RGB " {
                        if pos + 12 > data.len() {
                            pos = block_end;
                            continue;
                        }
                        let rf = f32::from_be_bytes([
                            data[pos],
                            data[pos + 1],
                            data[pos + 2],
                            data[pos + 3],
                        ]);
                        let gf = f32::from_be_bytes([
                            data[pos + 4],
                            data[pos + 5],
                            data[pos + 6],
                            data[pos + 7],
                        ]);
                        let bf = f32::from_be_bytes([
                            data[pos + 8],
                            data[pos + 9],
                            data[pos + 10],
                            data[pos + 11],
                        ]);
                        (
                            (rf.clamp(0.0, 1.0) * 255.0).round() as u8,
                            (gf.clamp(0.0, 1.0) * 255.0).round() as u8,
                            (bf.clamp(0.0, 1.0) * 255.0).round() as u8,
                        )
                    } else if model == b"Gray" {
                        if pos + 4 > data.len() {
                            pos = block_end;
                            continue;
                        }
                        let vf = f32::from_be_bytes([
                            data[pos],
                            data[pos + 1],
                            data[pos + 2],
                            data[pos + 3],
                        ]);
                        let v = (vf.clamp(0.0, 1.0) * 255.0).round() as u8;
                        (v, v, v)
                    } else if model == b"CMYK" {
                        if pos + 16 > data.len() {
                            pos = block_end;
                            continue;
                        }
                        let c = f32::from_be_bytes([
                            data[pos],
                            data[pos + 1],
                            data[pos + 2],
                            data[pos + 3],
                        ]);
                        let m = f32::from_be_bytes([
                            data[pos + 4],
                            data[pos + 5],
                            data[pos + 6],
                            data[pos + 7],
                        ]);
                        let y = f32::from_be_bytes([
                            data[pos + 8],
                            data[pos + 9],
                            data[pos + 10],
                            data[pos + 11],
                        ]);
                        let k = f32::from_be_bytes([
                            data[pos + 12],
                            data[pos + 13],
                            data[pos + 14],
                            data[pos + 15],
                        ]);
                        // CMYK to RGB
                        let r = ((1.0 - c) * (1.0 - k) * 255.0).round() as u8;
                        let g = ((1.0 - m) * (1.0 - k) * 255.0).round() as u8;
                        let b = ((1.0 - y) * (1.0 - k) * 255.0).round() as u8;
                        (r, g, b)
                    } else {
                        // Unknown model, skip
                        pos = block_end;
                        continue;
                    };

                    colors.push(Color { r, g, b, name });
                }
                BLOCK_GROUP_START | BLOCK_GROUP_END => {
                    // Skip group blocks for now
                }
                _ => {}
            }

            pos = block_end;
        }

        Ok(Palette { name: None, colors })
    }

    pub fn serialize_ase(palette: &Palette) -> Vec<u8> {
        let mut buf = Vec::new();

        // Magic + version
        buf.extend_from_slice(ASE_MAGIC);
        write_u32_be(&mut buf, 0x00010000); // version 1.0

        // Number of blocks (one per color)
        write_u32_be(&mut buf, palette.colors.len() as u32);

        for color in &palette.colors {
            let name = color.name.as_deref().unwrap_or("Untitled");
            // Encode name as UTF-16BE with null terminator
            let utf16: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            let name_len = utf16.len() as u16;

            // Block content: name_len(2) + name_bytes(name_len*2) + model(4) + RGB(12) + type(2)
            let block_content_len = 2 + (name_len as usize * 2) + 4 + 12 + 2;

            write_u16_be(&mut buf, BLOCK_COLOR);
            write_u32_be(&mut buf, block_content_len as u32);

            // Name
            write_u16_be(&mut buf, name_len);
            for code_unit in &utf16 {
                write_u16_be(&mut buf, *code_unit);
            }

            // Color model: RGB
            buf.extend_from_slice(b"RGB ");

            // RGB values as f32 (0.0..=1.0)
            write_f32_be(&mut buf, color.r as f32 / 255.0);
            write_f32_be(&mut buf, color.g as f32 / 255.0);
            write_f32_be(&mut buf, color.b as f32 / 255.0);

            // Color type: 2 = normal
            write_u16_be(&mut buf, 2);
        }

        buf
    }
}

#[cfg(feature = "ase")]
pub use ase_impl::{AseToJson, JsonToAse};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "gpl")]
    fn test_gpl_roundtrip() {
        use gpl_impl::{parse_gpl, serialize_gpl};

        let gpl = "GIMP Palette\nName: Test\nColumns: 16\n#\n255   0   0  Red\n  0 255   0  Green\n  0   0 255  Blue\n";
        let palette = parse_gpl(gpl).unwrap();
        assert_eq!(palette.name, Some("Test".to_string()));
        assert_eq!(palette.colors.len(), 3);
        assert_eq!(palette.colors[0].r, 255);
        assert_eq!(palette.colors[0].g, 0);
        assert_eq!(palette.colors[0].b, 0);

        let out = serialize_gpl(&palette);
        let palette2 = parse_gpl(&out).unwrap();
        assert_eq!(palette2.colors.len(), 3);
        assert_eq!(palette2.colors[0].r, 255);
    }

    #[test]
    #[cfg(feature = "aco")]
    fn test_aco_roundtrip() {
        use aco_impl::{parse_aco, serialize_aco};

        let palette = Palette {
            name: None,
            colors: vec![
                Color {
                    r: 255,
                    g: 0,
                    b: 0,
                    name: Some("Red".to_string()),
                },
                Color {
                    r: 0,
                    g: 255,
                    b: 0,
                    name: Some("Green".to_string()),
                },
                Color {
                    r: 0,
                    g: 0,
                    b: 255,
                    name: Some("Blue".to_string()),
                },
            ],
        };

        let aco = serialize_aco(&palette);
        let parsed = parse_aco(&aco).unwrap();
        assert_eq!(parsed.colors.len(), 3);
        assert_eq!(parsed.colors[0].r, 255);
        assert_eq!(parsed.colors[0].g, 0);
        assert_eq!(parsed.colors[0].b, 0);
        assert_eq!(parsed.colors[0].name, Some("Red".to_string()));
    }

    #[test]
    #[cfg(feature = "ase")]
    fn test_ase_roundtrip() {
        use ase_impl::{parse_ase, serialize_ase};

        let palette = Palette {
            name: None,
            colors: vec![
                Color {
                    r: 255,
                    g: 0,
                    b: 0,
                    name: Some("Red".to_string()),
                },
                Color {
                    r: 0,
                    g: 128,
                    b: 255,
                    name: Some("Sky Blue".to_string()),
                },
            ],
        };

        let ase = serialize_ase(&palette);
        let parsed = parse_ase(&ase).unwrap();
        assert_eq!(parsed.colors.len(), 2);
        assert_eq!(parsed.colors[0].r, 255);
        assert_eq!(parsed.colors[0].name, Some("Red".to_string()));
    }
}
