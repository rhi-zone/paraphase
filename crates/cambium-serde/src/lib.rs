//! Serde-based format converters for Cambium.
//!
//! This crate provides converters between various data serialization formats
//! using the serde ecosystem. Enable formats via feature flags.
//!
//! # Features
//!
//! ## Text formats (human-readable)
//! - `json` (default) - JSON via serde_json
//! - `yaml` (default) - YAML via serde_yaml
//! - `toml` (default) - TOML via toml
//! - `ron` - Rusty Object Notation
//! - `json5` - JSON5 (JSON with comments/trailing commas)
//! - `xml` - XML via quick-xml
//! - `lexpr` - S-expressions (Lisp-style)
//! - `urlencoded` - URL-encoded form data
//! - `qs` - Query strings
//! - `csv` - CSV (limited to arrays of flat objects)
//!
//! ## Binary formats (compact/efficient)
//! - `msgpack` - MessagePack binary format
//! - `cbor` - CBOR (RFC 8949)
//! - `bincode` - Fast binary encoding
//! - `postcard` - Embedded-friendly binary format
//! - `bson` - Binary JSON (MongoDB)
//! - `flexbuffers` - Schemaless FlatBuffers
//! - `bencode` - BitTorrent encoding
//! - `pickle` - Python's serialization format
//! - `plist` - Apple Property List
//!
//! ## Encoding formats (byte representations)
//! - `base64` - Base64 encoding/decoding
//! - `hex` - Hexadecimal encoding/decoding
//!
//! ## Line-based formats
//! - `ndjson` - Newline-delimited JSON (JSON Lines)
//!
//! ## Compression formats
//! - `gzip` - Gzip compression/decompression
//! - `zstd` - Zstandard compression/decompression
//! - `brotli` - Brotli compression/decompression
//!
//! ## Config formats
//! - `ini` - INI file format (bidirectional with JSON)
//!
//! ## Text transforms
//! - `charsets` - Character encoding conversion (UTF-16, Latin-1, etc.)
//! - `markdown` - Markdown to HTML conversion
//! - `html2text` - HTML to plain text conversion
//!
//! ## Feature group
//! - `all` - All formats

use cambium::{
    ConvertError, ConvertOutput, Converter, ConverterDecl, Properties, PropertyPattern, Registry,
};

/// Register all enabled serde converters with the registry.
pub fn register_all(registry: &mut Registry) {
    let formats = enabled_formats();

    // Register converters between all pairs of enabled formats
    for from in &formats {
        for to in &formats {
            if from != to {
                registry.register(SerdeConverter::new(from, to));
            }
        }
    }

    // Register encoding converters (base64, hex)
    #[cfg(feature = "base64")]
    {
        registry.register(Base64Encoder);
        registry.register(Base64Decoder);
    }
    #[cfg(feature = "hex")]
    {
        registry.register(HexEncoder);
        registry.register(HexDecoder);
    }

    // Register NDJSON converters
    #[cfg(feature = "ndjson")]
    {
        registry.register(JsonToNdjson);
        registry.register(NdjsonToJson);
    }

    // Register compression converters
    #[cfg(feature = "gzip")]
    {
        registry.register(GzipCompress);
        registry.register(GzipDecompress);
    }
    #[cfg(feature = "zstd")]
    {
        registry.register(ZstdCompress);
        registry.register(ZstdDecompress);
    }
    #[cfg(feature = "brotli")]
    {
        registry.register(BrotliCompress);
        registry.register(BrotliDecompress);
    }

    // Register config format converters
    #[cfg(feature = "ini")]
    {
        registry.register(IniToJson);
        registry.register(JsonToIni);
    }

    // Register charset converters
    #[cfg(feature = "charsets")]
    {
        registry.register(CharsetToUtf8);
        registry.register(Utf8ToCharset);
    }

    // Register text transform converters
    #[cfg(feature = "markdown")]
    {
        registry.register(MarkdownToHtml);
    }
    #[cfg(feature = "html2text")]
    {
        registry.register(HtmlToText);
    }
}

/// Get list of enabled formats based on feature flags.
pub fn enabled_formats() -> Vec<&'static str> {
    [
        // Text formats
        #[cfg(feature = "json")]
        "json",
        #[cfg(feature = "yaml")]
        "yaml",
        #[cfg(feature = "toml")]
        "toml",
        #[cfg(feature = "ron")]
        "ron",
        #[cfg(feature = "json5")]
        "json5",
        #[cfg(feature = "xml")]
        "xml",
        #[cfg(feature = "lexpr")]
        "lexpr",
        #[cfg(feature = "urlencoded")]
        "urlencoded",
        #[cfg(feature = "qs")]
        "qs",
        // Binary formats
        #[cfg(feature = "msgpack")]
        "msgpack",
        #[cfg(feature = "cbor")]
        "cbor",
        #[cfg(feature = "bincode")]
        "bincode",
        #[cfg(feature = "postcard")]
        "postcard",
        #[cfg(feature = "bson")]
        "bson",
        #[cfg(feature = "flexbuffers")]
        "flexbuffers",
        #[cfg(feature = "bencode")]
        "bencode",
        #[cfg(feature = "pickle")]
        "pickle",
        #[cfg(feature = "plist")]
        "plist",
        // CSV is special - only works with arrays of flat objects
        // Don't include in general conversion matrix
    ]
    .into()
}

/// A converter between two serde-compatible formats.
pub struct SerdeConverter {
    decl: ConverterDecl,
    from: &'static str,
    to: &'static str,
}

impl SerdeConverter {
    pub fn new(from: &'static str, to: &'static str) -> Self {
        let id = format!("serde.{}-to-{}", from, to);
        let decl = ConverterDecl::simple(
            &id,
            PropertyPattern::new().eq("format", from),
            PropertyPattern::new().eq("format", to),
        )
        .description(format!(
            "Convert {} to {} via serde",
            from.to_uppercase(),
            to.to_uppercase()
        ));

        Self { decl, from, to }
    }
}

impl Converter for SerdeConverter {
    fn decl(&self) -> &ConverterDecl {
        &self.decl
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        // Deserialize from source format
        let value: serde_json::Value = deserialize(self.from, input)?;

        // Serialize to target format
        let output = serialize(self.to, &value)?;

        // Update properties
        let mut out_props = props.clone();
        out_props.insert("format".into(), self.to.into());

        Ok(ConvertOutput::Single(output, out_props))
    }
}

// ============================================
// Base64 encoding/decoding
// ============================================

#[cfg(feature = "base64")]
mod base64_impl {
    use super::*;
    use base64::prelude::*;

    /// Encode raw bytes to base64 text.
    pub struct Base64Encoder;

    impl Converter for Base64Encoder {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "encoding.raw-to-base64",
                    PropertyPattern::new().eq("format", "raw"),
                    PropertyPattern::new().eq("format", "base64"),
                )
                .description("Encode raw bytes to base64")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let encoded = BASE64_STANDARD.encode(input);
            let mut out_props = props.clone();
            out_props.insert("format".into(), "base64".into());
            Ok(ConvertOutput::Single(encoded.into_bytes(), out_props))
        }
    }

    /// Decode base64 text to raw bytes.
    pub struct Base64Decoder;

    impl Converter for Base64Decoder {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "encoding.base64-to-raw",
                    PropertyPattern::new().eq("format", "base64"),
                    PropertyPattern::new().eq("format", "raw"),
                )
                .description("Decode base64 to raw bytes")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            // Handle input as text (trim whitespace)
            let text = std::str::from_utf8(input)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?
                .trim();
            let decoded = BASE64_STANDARD
                .decode(text)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid base64: {}", e)))?;
            let mut out_props = props.clone();
            out_props.insert("format".into(), "raw".into());
            Ok(ConvertOutput::Single(decoded, out_props))
        }
    }
}

#[cfg(feature = "base64")]
pub use base64_impl::{Base64Decoder, Base64Encoder};

// ============================================
// Hex encoding/decoding
// ============================================

#[cfg(feature = "hex")]
mod hex_impl {
    use super::*;

    /// Encode raw bytes to hexadecimal text.
    pub struct HexEncoder;

    impl Converter for HexEncoder {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "encoding.raw-to-hex",
                    PropertyPattern::new().eq("format", "raw"),
                    PropertyPattern::new().eq("format", "hex"),
                )
                .description("Encode raw bytes to hexadecimal")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let encoded = hex::encode(input);
            let mut out_props = props.clone();
            out_props.insert("format".into(), "hex".into());
            Ok(ConvertOutput::Single(encoded.into_bytes(), out_props))
        }
    }

    /// Decode hexadecimal text to raw bytes.
    pub struct HexDecoder;

    impl Converter for HexDecoder {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "encoding.hex-to-raw",
                    PropertyPattern::new().eq("format", "hex"),
                    PropertyPattern::new().eq("format", "raw"),
                )
                .description("Decode hexadecimal to raw bytes")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            // Handle input as text (trim whitespace, remove common separators)
            let text = std::str::from_utf8(input)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?
                .trim()
                .replace([' ', ':', '-'], "");
            let decoded = hex::decode(&text)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid hex: {}", e)))?;
            let mut out_props = props.clone();
            out_props.insert("format".into(), "raw".into());
            Ok(ConvertOutput::Single(decoded, out_props))
        }
    }
}

#[cfg(feature = "hex")]
pub use hex_impl::{HexDecoder, HexEncoder};

// ============================================
// NDJSON (Newline-delimited JSON)
// ============================================

#[cfg(feature = "ndjson")]
mod ndjson_impl {
    use super::*;

    /// Convert JSON array to newline-delimited JSON.
    pub struct JsonToNdjson;

    impl Converter for JsonToNdjson {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "serde.json-to-ndjson",
                    PropertyPattern::new().eq("format", "json"),
                    PropertyPattern::new().eq("format", "ndjson"),
                )
                .description("Convert JSON array to newline-delimited JSON")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let value: serde_json::Value = serde_json::from_slice(input)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid JSON: {}", e)))?;

            let array = value
                .as_array()
                .ok_or_else(|| ConvertError::InvalidInput("JSON must be an array".into()))?;

            let mut output = Vec::new();
            for item in array {
                serde_json::to_writer(&mut output, item).map_err(|e| {
                    ConvertError::Failed(format!("JSON serialization failed: {}", e))
                })?;
                output.push(b'\n');
            }

            let mut out_props = props.clone();
            out_props.insert("format".into(), "ndjson".into());
            Ok(ConvertOutput::Single(output, out_props))
        }
    }

    /// Convert newline-delimited JSON to JSON array.
    pub struct NdjsonToJson;

    impl Converter for NdjsonToJson {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "serde.ndjson-to-json",
                    PropertyPattern::new().eq("format", "ndjson"),
                    PropertyPattern::new().eq("format", "json"),
                )
                .description("Convert newline-delimited JSON to JSON array")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let text = std::str::from_utf8(input)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?;

            let mut items = Vec::new();
            for (line_num, line) in text.lines().enumerate() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let item: serde_json::Value = serde_json::from_str(line).map_err(|e| {
                    ConvertError::InvalidInput(format!(
                        "Invalid JSON at line {}: {}",
                        line_num + 1,
                        e
                    ))
                })?;
                items.push(item);
            }

            let array = serde_json::Value::Array(items);
            let output = serde_json::to_vec_pretty(&array)
                .map_err(|e| ConvertError::Failed(format!("JSON serialization failed: {}", e)))?;

            let mut out_props = props.clone();
            out_props.insert("format".into(), "json".into());
            Ok(ConvertOutput::Single(output, out_props))
        }
    }
}

#[cfg(feature = "ndjson")]
pub use ndjson_impl::{JsonToNdjson, NdjsonToJson};

// ============================================
// Compression (gzip, zstd, brotli)
// ============================================

#[cfg(feature = "gzip")]
mod gzip_impl {
    use super::*;
    use flate2::Compression;
    use flate2::read::{GzDecoder, GzEncoder};
    use std::io::Read;

    /// Compress raw bytes with gzip.
    pub struct GzipCompress;

    impl Converter for GzipCompress {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "compression.raw-to-gzip",
                    PropertyPattern::new().eq("format", "raw"),
                    PropertyPattern::new().eq("format", "gzip"),
                )
                .description("Compress with gzip")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let mut encoder = GzEncoder::new(input, Compression::default());
            let mut output = Vec::new();
            encoder
                .read_to_end(&mut output)
                .map_err(|e| ConvertError::Failed(format!("Gzip compression failed: {}", e)))?;
            let mut out_props = props.clone();
            out_props.insert("format".into(), "gzip".into());
            Ok(ConvertOutput::Single(output, out_props))
        }
    }

    /// Decompress gzip bytes.
    pub struct GzipDecompress;

    impl Converter for GzipDecompress {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "compression.gzip-to-raw",
                    PropertyPattern::new().eq("format", "gzip"),
                    PropertyPattern::new().eq("format", "raw"),
                )
                .description("Decompress gzip")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let mut decoder = GzDecoder::new(input);
            let mut output = Vec::new();
            decoder.read_to_end(&mut output).map_err(|e| {
                ConvertError::InvalidInput(format!("Gzip decompression failed: {}", e))
            })?;
            let mut out_props = props.clone();
            out_props.insert("format".into(), "raw".into());
            Ok(ConvertOutput::Single(output, out_props))
        }
    }
}

#[cfg(feature = "gzip")]
pub use gzip_impl::{GzipCompress, GzipDecompress};

#[cfg(feature = "zstd")]
mod zstd_impl {
    use super::*;

    /// Compress raw bytes with zstd.
    pub struct ZstdCompress;

    impl Converter for ZstdCompress {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "compression.raw-to-zstd",
                    PropertyPattern::new().eq("format", "raw"),
                    PropertyPattern::new().eq("format", "zstd"),
                )
                .description("Compress with zstd")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let output = zstd::encode_all(input, 0)
                .map_err(|e| ConvertError::Failed(format!("Zstd compression failed: {}", e)))?;
            let mut out_props = props.clone();
            out_props.insert("format".into(), "zstd".into());
            Ok(ConvertOutput::Single(output, out_props))
        }
    }

    /// Decompress zstd bytes.
    pub struct ZstdDecompress;

    impl Converter for ZstdDecompress {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "compression.zstd-to-raw",
                    PropertyPattern::new().eq("format", "zstd"),
                    PropertyPattern::new().eq("format", "raw"),
                )
                .description("Decompress zstd")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let output = zstd::decode_all(input).map_err(|e| {
                ConvertError::InvalidInput(format!("Zstd decompression failed: {}", e))
            })?;
            let mut out_props = props.clone();
            out_props.insert("format".into(), "raw".into());
            Ok(ConvertOutput::Single(output, out_props))
        }
    }
}

#[cfg(feature = "zstd")]
pub use zstd_impl::{ZstdCompress, ZstdDecompress};

#[cfg(feature = "brotli")]
mod brotli_impl {
    use super::*;
    use std::io::Read;

    /// Compress raw bytes with brotli.
    pub struct BrotliCompress;

    impl Converter for BrotliCompress {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "compression.raw-to-brotli",
                    PropertyPattern::new().eq("format", "raw"),
                    PropertyPattern::new().eq("format", "brotli"),
                )
                .description("Compress with brotli")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let mut output = Vec::new();
            let mut compressor = brotli::CompressorReader::new(input, 4096, 6, 22);
            compressor
                .read_to_end(&mut output)
                .map_err(|e| ConvertError::Failed(format!("Brotli compression failed: {}", e)))?;
            let mut out_props = props.clone();
            out_props.insert("format".into(), "brotli".into());
            Ok(ConvertOutput::Single(output, out_props))
        }
    }

    /// Decompress brotli bytes.
    pub struct BrotliDecompress;

    impl Converter for BrotliDecompress {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "compression.brotli-to-raw",
                    PropertyPattern::new().eq("format", "brotli"),
                    PropertyPattern::new().eq("format", "raw"),
                )
                .description("Decompress brotli")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let mut output = Vec::new();
            let mut decompressor = brotli::Decompressor::new(input, 4096);
            decompressor.read_to_end(&mut output).map_err(|e| {
                ConvertError::InvalidInput(format!("Brotli decompression failed: {}", e))
            })?;
            let mut out_props = props.clone();
            out_props.insert("format".into(), "raw".into());
            Ok(ConvertOutput::Single(output, out_props))
        }
    }
}

#[cfg(feature = "brotli")]
pub use brotli_impl::{BrotliCompress, BrotliDecompress};

// ============================================
// INI config format
// ============================================

#[cfg(feature = "ini")]
mod ini_impl {
    use super::*;
    use ini::Ini;

    /// Convert INI to JSON.
    pub struct IniToJson;

    impl Converter for IniToJson {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "config.ini-to-json",
                    PropertyPattern::new().eq("format", "ini"),
                    PropertyPattern::new().eq("format", "json"),
                )
                .description("Convert INI to JSON")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let text = std::str::from_utf8(input)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?;
            let ini = Ini::load_from_str(text)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid INI: {}", e)))?;

            // Convert to JSON object
            let mut root = serde_json::Map::new();
            for (section, properties) in ini.iter() {
                let section_name = section.unwrap_or("_global");
                let mut section_obj = serde_json::Map::new();
                for (key, value) in properties.iter() {
                    section_obj.insert(
                        key.to_string(),
                        serde_json::Value::String(value.to_string()),
                    );
                }
                root.insert(
                    section_name.to_string(),
                    serde_json::Value::Object(section_obj),
                );
            }

            let output = serde_json::to_vec_pretty(&root)
                .map_err(|e| ConvertError::Failed(format!("JSON serialization failed: {}", e)))?;

            let mut out_props = props.clone();
            out_props.insert("format".into(), "json".into());
            Ok(ConvertOutput::Single(output, out_props))
        }
    }

    /// Convert JSON to INI.
    pub struct JsonToIni;

    impl Converter for JsonToIni {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "config.json-to-ini",
                    PropertyPattern::new().eq("format", "json"),
                    PropertyPattern::new().eq("format", "ini"),
                )
                .description("Convert JSON to INI")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let value: serde_json::Value = serde_json::from_slice(input)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid JSON: {}", e)))?;

            let obj = value
                .as_object()
                .ok_or_else(|| ConvertError::InvalidInput("JSON must be an object".into()))?;

            let mut ini = Ini::new();
            for (section, section_value) in obj {
                let section_name = if section == "_global" {
                    None
                } else {
                    Some(section.as_str())
                };
                if let Some(section_obj) = section_value.as_object() {
                    for (key, val) in section_obj {
                        let str_val = match val {
                            serde_json::Value::String(s) => s.clone(),
                            other => other.to_string(),
                        };
                        ini.with_section(section_name).set(key, str_val);
                    }
                }
            }

            let mut output = Vec::new();
            ini.write_to(&mut output)
                .map_err(|e| ConvertError::Failed(format!("INI serialization failed: {}", e)))?;

            let mut out_props = props.clone();
            out_props.insert("format".into(), "ini".into());
            Ok(ConvertOutput::Single(output, out_props))
        }
    }
}

#[cfg(feature = "ini")]
pub use ini_impl::{IniToJson, JsonToIni};

// ============================================
// Character encoding conversion
// ============================================

#[cfg(feature = "charsets")]
mod charsets_impl {
    use super::*;

    /// Convert from a character encoding to UTF-8.
    pub struct CharsetToUtf8;

    impl Converter for CharsetToUtf8 {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "encoding.charset-to-utf8",
                    PropertyPattern::new().exists("charset"),
                    PropertyPattern::new().eq("charset", "utf-8"),
                )
                .description("Convert character encoding to UTF-8")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let charset = props
                .get("charset")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ConvertError::InvalidInput("Missing 'charset' property".into()))?;

            let encoding =
                encoding_rs::Encoding::for_label(charset.as_bytes()).ok_or_else(|| {
                    ConvertError::InvalidInput(format!("Unknown charset: {}", charset))
                })?;

            let (decoded, _, had_errors) = encoding.decode(input);
            if had_errors {
                return Err(ConvertError::InvalidInput(format!(
                    "Invalid {} sequence in input",
                    charset
                )));
            }

            let mut out_props = props.clone();
            out_props.insert("charset".into(), "utf-8".into());
            Ok(ConvertOutput::Single(
                decoded.into_owned().into_bytes(),
                out_props,
            ))
        }
    }

    /// Convert from UTF-8 to another character encoding.
    pub struct Utf8ToCharset;

    impl Converter for Utf8ToCharset {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "encoding.utf8-to-charset",
                    PropertyPattern::new().eq("charset", "utf-8"),
                    PropertyPattern::new().exists("target_charset"),
                )
                .description("Convert UTF-8 to another character encoding")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let target = props
                .get("target_charset")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ConvertError::InvalidInput("Missing 'target_charset' property".into())
                })?;

            let text = std::str::from_utf8(input)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?;

            let encoding =
                encoding_rs::Encoding::for_label(target.as_bytes()).ok_or_else(|| {
                    ConvertError::InvalidInput(format!("Unknown charset: {}", target))
                })?;

            let (encoded, _, had_errors) = encoding.encode(text);
            if had_errors {
                return Err(ConvertError::Failed(format!(
                    "Cannot encode to {}: input contains unmappable characters",
                    target
                )));
            }

            let mut out_props = props.clone();
            out_props.insert("charset".into(), target.into());
            out_props.shift_remove("target_charset");
            Ok(ConvertOutput::Single(encoded.into_owned(), out_props))
        }
    }
}

#[cfg(feature = "charsets")]
pub use charsets_impl::{CharsetToUtf8, Utf8ToCharset};

// ============================================
// Markdown → HTML
// ============================================

#[cfg(feature = "markdown")]
mod markdown_impl {
    use super::*;
    use pulldown_cmark::{Parser, html};

    /// Convert Markdown to HTML.
    pub struct MarkdownToHtml;

    impl Converter for MarkdownToHtml {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "text.markdown-to-html",
                    PropertyPattern::new().eq("format", "markdown"),
                    PropertyPattern::new().eq("format", "html"),
                )
                .description("Convert Markdown to HTML")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let text = std::str::from_utf8(input)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?;

            let parser = Parser::new(text);
            let mut html_output = String::new();
            html::push_html(&mut html_output, parser);

            let mut out_props = props.clone();
            out_props.insert("format".into(), "html".into());
            Ok(ConvertOutput::Single(html_output.into_bytes(), out_props))
        }
    }
}

#[cfg(feature = "markdown")]
pub use markdown_impl::MarkdownToHtml;

// ============================================
// HTML → Plain text
// ============================================

#[cfg(feature = "html2text")]
mod html2text_impl {
    use super::*;

    /// Convert HTML to plain text.
    pub struct HtmlToText;

    impl Converter for HtmlToText {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "text.html-to-text",
                    PropertyPattern::new().eq("format", "html"),
                    PropertyPattern::new().eq("format", "text"),
                )
                .description("Convert HTML to plain text")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let text = html2text::from_read(input, 80)
                .map_err(|e| ConvertError::InvalidInput(format!("HTML parsing failed: {}", e)))?;

            let mut out_props = props.clone();
            out_props.insert("format".into(), "text".into());
            Ok(ConvertOutput::Single(text.into_bytes(), out_props))
        }
    }
}

#[cfg(feature = "html2text")]
pub use html2text_impl::HtmlToText;

/// Deserialize bytes to a serde Value.
fn deserialize(format: &str, data: &[u8]) -> Result<serde_json::Value, ConvertError> {
    match format {
        // === Text formats ===
        #[cfg(feature = "json")]
        "json" => serde_json::from_slice(data)
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid JSON: {}", e))),

        #[cfg(feature = "yaml")]
        "yaml" => serde_yaml::from_slice(data)
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid YAML: {}", e))),

        #[cfg(feature = "toml")]
        "toml" => {
            let s = std::str::from_utf8(data)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?;
            toml::from_str(s)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid TOML: {}", e)))
        }

        #[cfg(feature = "ron")]
        "ron" => {
            let s = std::str::from_utf8(data)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?;
            ron::from_str(s).map_err(|e| ConvertError::InvalidInput(format!("Invalid RON: {}", e)))
        }

        #[cfg(feature = "json5")]
        "json5" => {
            let s = std::str::from_utf8(data)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?;
            json5::from_str(s)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid JSON5: {}", e)))
        }

        #[cfg(feature = "xml")]
        "xml" => {
            let s = std::str::from_utf8(data)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?;
            quick_xml::de::from_str(s)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid XML: {}", e)))
        }

        #[cfg(feature = "lexpr")]
        "lexpr" => {
            let s = std::str::from_utf8(data)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?;
            serde_lexpr::from_str(s)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid S-expression: {}", e)))
        }

        #[cfg(feature = "urlencoded")]
        "urlencoded" => {
            let s = std::str::from_utf8(data)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?;
            serde_urlencoded::from_str(s)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid URL-encoded: {}", e)))
        }

        #[cfg(feature = "qs")]
        "qs" => {
            let s = std::str::from_utf8(data)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?;
            serde_qs::from_str(s)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid query string: {}", e)))
        }

        // === Binary formats ===
        #[cfg(feature = "msgpack")]
        "msgpack" => rmp_serde::from_slice(data)
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid MessagePack: {}", e))),

        #[cfg(feature = "cbor")]
        "cbor" => ciborium::from_reader(data)
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid CBOR: {}", e))),

        #[cfg(feature = "bincode")]
        "bincode" => {
            let (value, _): (serde_json::Value, _) =
                bincode::serde::decode_from_slice(data, bincode::config::standard())
                    .map_err(|e| ConvertError::InvalidInput(format!("Invalid Bincode: {}", e)))?;
            Ok(value)
        }

        #[cfg(feature = "postcard")]
        "postcard" => postcard::from_bytes(data)
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid Postcard: {}", e))),

        #[cfg(feature = "bson")]
        "bson" => bson::de::deserialize_from_slice(data)
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid BSON: {}", e))),

        #[cfg(feature = "flexbuffers")]
        "flexbuffers" => flexbuffers::from_slice(data)
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid FlexBuffers: {}", e))),

        #[cfg(feature = "bencode")]
        "bencode" => serde_bencode::from_bytes(data)
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid Bencode: {}", e))),

        #[cfg(feature = "pickle")]
        "pickle" => serde_pickle::from_slice(data, serde_pickle::DeOptions::default())
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid Pickle: {}", e))),

        #[cfg(feature = "plist")]
        "plist" => plist::from_bytes(data)
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid Property List: {}", e))),

        _ => Err(ConvertError::Failed(format!(
            "Unsupported source format: {}",
            format
        ))),
    }
}

/// Serialize a serde Value to bytes.
fn serialize(format: &str, value: &serde_json::Value) -> Result<Vec<u8>, ConvertError> {
    match format {
        // === Text formats ===
        #[cfg(feature = "json")]
        "json" => serde_json::to_vec_pretty(value)
            .map_err(|e| ConvertError::Failed(format!("JSON serialization failed: {}", e))),

        #[cfg(feature = "yaml")]
        "yaml" => serde_yaml::to_string(value)
            .map(|s| s.into_bytes())
            .map_err(|e| ConvertError::Failed(format!("YAML serialization failed: {}", e))),

        #[cfg(feature = "toml")]
        "toml" => toml::to_string_pretty(value)
            .map(|s| s.into_bytes())
            .map_err(|e| ConvertError::Failed(format!("TOML serialization failed: {}", e))),

        #[cfg(feature = "ron")]
        "ron" => ron::to_string(value)
            .map(|s| s.into_bytes())
            .map_err(|e| ConvertError::Failed(format!("RON serialization failed: {}", e))),

        #[cfg(feature = "json5")]
        "json5" => {
            // json5 crate doesn't have serialization, output as JSON (compatible)
            serde_json::to_vec_pretty(value)
                .map_err(|e| ConvertError::Failed(format!("JSON5 serialization failed: {}", e)))
        }

        #[cfg(feature = "xml")]
        "xml" => quick_xml::se::to_string(value)
            .map(|s| s.into_bytes())
            .map_err(|e| ConvertError::Failed(format!("XML serialization failed: {}", e))),

        #[cfg(feature = "lexpr")]
        "lexpr" => serde_lexpr::to_string(value)
            .map(|s| s.into_bytes())
            .map_err(|e| ConvertError::Failed(format!("S-expression serialization failed: {}", e))),

        #[cfg(feature = "urlencoded")]
        "urlencoded" => serde_urlencoded::to_string(value)
            .map(|s| s.into_bytes())
            .map_err(|e| ConvertError::Failed(format!("URL-encoded serialization failed: {}", e))),

        #[cfg(feature = "qs")]
        "qs" => serde_qs::to_string(value)
            .map(|s| s.into_bytes())
            .map_err(|e| ConvertError::Failed(format!("Query string serialization failed: {}", e))),

        // === Binary formats ===
        #[cfg(feature = "msgpack")]
        "msgpack" => rmp_serde::to_vec(value)
            .map_err(|e| ConvertError::Failed(format!("MessagePack serialization failed: {}", e))),

        #[cfg(feature = "cbor")]
        "cbor" => {
            let mut buf = Vec::new();
            ciborium::into_writer(value, &mut buf)
                .map_err(|e| ConvertError::Failed(format!("CBOR serialization failed: {}", e)))?;
            Ok(buf)
        }

        #[cfg(feature = "bincode")]
        "bincode" => bincode::serde::encode_to_vec(value, bincode::config::standard())
            .map_err(|e| ConvertError::Failed(format!("Bincode serialization failed: {}", e))),

        #[cfg(feature = "postcard")]
        "postcard" => postcard::to_allocvec(value)
            .map_err(|e| ConvertError::Failed(format!("Postcard serialization failed: {}", e))),

        #[cfg(feature = "bson")]
        "bson" => bson::ser::serialize_to_vec(value)
            .map_err(|e| ConvertError::Failed(format!("BSON serialization failed: {}", e))),

        #[cfg(feature = "flexbuffers")]
        "flexbuffers" => flexbuffers::to_vec(value)
            .map_err(|e| ConvertError::Failed(format!("FlexBuffers serialization failed: {}", e))),

        #[cfg(feature = "bencode")]
        "bencode" => serde_bencode::to_bytes(value)
            .map_err(|e| ConvertError::Failed(format!("Bencode serialization failed: {}", e))),

        #[cfg(feature = "pickle")]
        "pickle" => serde_pickle::to_vec(value, serde_pickle::SerOptions::default())
            .map_err(|e| ConvertError::Failed(format!("Pickle serialization failed: {}", e))),

        #[cfg(feature = "plist")]
        "plist" => {
            let mut buf = Vec::new();
            plist::to_writer_binary(&mut buf, value).map_err(|e| {
                ConvertError::Failed(format!("Property List serialization failed: {}", e))
            })?;
            Ok(buf)
        }

        _ => Err(ConvertError::Failed(format!(
            "Unsupported target format: {}",
            format
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cambium::PropertiesExt;

    #[test]
    #[cfg(all(feature = "json", feature = "yaml"))]
    fn test_json_to_yaml() {
        let converter = SerdeConverter::new("json", "yaml");
        let input = br#"{"name": "test", "value": 42}"#;
        let props = Properties::new().with("format", "json");

        let result = converter.convert(input, &props).unwrap();

        match result {
            ConvertOutput::Single(output, out_props) => {
                let output_str = String::from_utf8(output).unwrap();
                assert!(output_str.contains("name:"));
                assert!(output_str.contains("test"));
                assert_eq!(out_props.get("format").unwrap().as_str(), Some("yaml"));
            }
            _ => panic!("Expected single output"),
        }
    }

    #[test]
    #[cfg(all(feature = "yaml", feature = "json"))]
    fn test_yaml_to_json() {
        let converter = SerdeConverter::new("yaml", "json");
        let input = b"name: test\nvalue: 42\n";
        let props = Properties::new().with("format", "yaml");

        let result = converter.convert(input, &props).unwrap();

        match result {
            ConvertOutput::Single(output, out_props) => {
                let output_str = String::from_utf8(output).unwrap();
                assert!(output_str.contains("\"name\""));
                assert!(output_str.contains("\"test\""));
                assert_eq!(out_props.get("format").unwrap().as_str(), Some("json"));
            }
            _ => panic!("Expected single output"),
        }
    }

    #[test]
    #[cfg(all(feature = "json", feature = "toml"))]
    fn test_json_to_toml() {
        let converter = SerdeConverter::new("json", "toml");
        let input = br#"{"name": "test", "value": 42}"#;
        let props = Properties::new().with("format", "json");

        let result = converter.convert(input, &props).unwrap();

        match result {
            ConvertOutput::Single(output, out_props) => {
                let output_str = String::from_utf8(output).unwrap();
                assert!(output_str.contains("name"));
                assert!(output_str.contains("test"));
                assert_eq!(out_props.get("format").unwrap().as_str(), Some("toml"));
            }
            _ => panic!("Expected single output"),
        }
    }

    #[test]
    fn test_register_all() {
        let mut registry = Registry::new();
        register_all(&mut registry);

        // Should have n*(n-1) serde converters for n formats
        let n = enabled_formats().len();
        let mut expected = n * (n - 1);

        // Plus encoding converters
        #[cfg(feature = "base64")]
        {
            expected += 2;
        }
        #[cfg(feature = "hex")]
        {
            expected += 2;
        }
        #[cfg(feature = "ndjson")]
        {
            expected += 2;
        }

        // Plus compression converters
        #[cfg(feature = "gzip")]
        {
            expected += 2;
        }
        #[cfg(feature = "zstd")]
        {
            expected += 2;
        }
        #[cfg(feature = "brotli")]
        {
            expected += 2;
        }

        // Plus config format converters
        #[cfg(feature = "ini")]
        {
            expected += 2;
        }

        // Plus charset converters
        #[cfg(feature = "charsets")]
        {
            expected += 2;
        }

        // Plus text transform converters
        #[cfg(feature = "markdown")]
        {
            expected += 1;
        }
        #[cfg(feature = "html2text")]
        {
            expected += 1;
        }

        assert_eq!(registry.len(), expected);
    }

    #[test]
    #[cfg(feature = "base64")]
    fn test_base64_roundtrip() {
        use crate::{Base64Decoder, Base64Encoder};

        let original = b"Hello, World! \x00\x01\x02\xff";
        let props = Properties::new().with("format", "raw");

        // Encode
        let encoded_result = Base64Encoder.convert(original, &props).unwrap();
        let (encoded, encoded_props) = match encoded_result {
            ConvertOutput::Single(b, p) => (b, p),
            _ => panic!("Expected single"),
        };
        assert_eq!(
            encoded_props.get("format").unwrap().as_str(),
            Some("base64")
        );
        assert_eq!(
            String::from_utf8(encoded.clone()).unwrap(),
            "SGVsbG8sIFdvcmxkISAAAQL/"
        );

        // Decode
        let decoded_result = Base64Decoder.convert(&encoded, &encoded_props).unwrap();
        let (decoded, _) = match decoded_result {
            ConvertOutput::Single(b, p) => (b, p),
            _ => panic!("Expected single"),
        };
        assert_eq!(decoded, original);
    }

    #[test]
    #[cfg(feature = "hex")]
    fn test_hex_roundtrip() {
        use crate::{HexDecoder, HexEncoder};

        let original = b"\xde\xad\xbe\xef";
        let props = Properties::new().with("format", "raw");

        // Encode
        let encoded_result = HexEncoder.convert(original, &props).unwrap();
        let (encoded, encoded_props) = match encoded_result {
            ConvertOutput::Single(b, p) => (b, p),
            _ => panic!("Expected single"),
        };
        assert_eq!(encoded_props.get("format").unwrap().as_str(), Some("hex"));
        assert_eq!(String::from_utf8(encoded.clone()).unwrap(), "deadbeef");

        // Decode
        let decoded_result = HexDecoder.convert(&encoded, &encoded_props).unwrap();
        let (decoded, _) = match decoded_result {
            ConvertOutput::Single(b, p) => (b, p),
            _ => panic!("Expected single"),
        };
        assert_eq!(decoded, original);
    }

    #[test]
    #[cfg(feature = "hex")]
    fn test_hex_with_separators() {
        use crate::HexDecoder;

        // Hex with various separators
        let input = b"de:ad:be:ef";
        let props = Properties::new().with("format", "hex");

        let result = HexDecoder.convert(input, &props).unwrap();
        let (decoded, _) = match result {
            ConvertOutput::Single(b, p) => (b, p),
            _ => panic!("Expected single"),
        };
        assert_eq!(decoded, b"\xde\xad\xbe\xef");
    }

    #[test]
    #[cfg(feature = "ndjson")]
    fn test_json_to_ndjson() {
        use crate::JsonToNdjson;

        let input = br#"[{"a": 1}, {"b": 2}, {"c": 3}]"#;
        let props = Properties::new().with("format", "json");

        let result = JsonToNdjson.convert(input, &props).unwrap();
        let (output, out_props) = match result {
            ConvertOutput::Single(b, p) => (b, p),
            _ => panic!("Expected single"),
        };

        let output_str = String::from_utf8(output).unwrap();
        assert_eq!(out_props.get("format").unwrap().as_str(), Some("ndjson"));
        assert!(output_str.contains(r#"{"a":1}"#));
        assert!(output_str.contains(r#"{"b":2}"#));
        assert!(output_str.contains(r#"{"c":3}"#));
        assert_eq!(output_str.lines().count(), 3);
    }

    #[test]
    #[cfg(feature = "ndjson")]
    fn test_ndjson_to_json() {
        use crate::NdjsonToJson;

        let input = b"{\"a\": 1}\n{\"b\": 2}\n{\"c\": 3}\n";
        let props = Properties::new().with("format", "ndjson");

        let result = NdjsonToJson.convert(input, &props).unwrap();
        let (output, out_props) = match result {
            ConvertOutput::Single(b, p) => (b, p),
            _ => panic!("Expected single"),
        };

        let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(out_props.get("format").unwrap().as_str(), Some("json"));
        assert!(value.is_array());
        assert_eq!(value.as_array().unwrap().len(), 3);
    }

    #[test]
    #[cfg(all(feature = "json", feature = "yaml"))]
    fn test_roundtrip() {
        let original = br#"{"name": "roundtrip", "nested": {"a": 1, "b": 2}}"#;

        let json_to_yaml = SerdeConverter::new("json", "yaml");
        let yaml_to_json = SerdeConverter::new("yaml", "json");

        let props = Properties::new().with("format", "json");

        // JSON -> YAML
        let yaml_result = json_to_yaml.convert(original, &props).unwrap();
        let (yaml_bytes, yaml_props) = match yaml_result {
            ConvertOutput::Single(b, p) => (b, p),
            _ => panic!("Expected single"),
        };

        // YAML -> JSON
        let json_result = yaml_to_json.convert(&yaml_bytes, &yaml_props).unwrap();
        let (json_bytes, _) = match json_result {
            ConvertOutput::Single(b, p) => (b, p),
            _ => panic!("Expected single"),
        };

        // Parse both and compare
        let original_value: serde_json::Value = serde_json::from_slice(original).unwrap();
        let roundtrip_value: serde_json::Value = serde_json::from_slice(&json_bytes).unwrap();
        assert_eq!(original_value, roundtrip_value);
    }

    #[test]
    #[cfg(feature = "gzip")]
    fn test_gzip_roundtrip() {
        use crate::{GzipCompress, GzipDecompress};

        let original = b"Hello, World! This is test data. ".repeat(100);
        let props = Properties::new().with("format", "raw");

        // Compress
        let compressed = GzipCompress.convert(&original, &props).unwrap();
        let (compressed_bytes, compressed_props) = match compressed {
            ConvertOutput::Single(b, p) => (b, p),
            _ => panic!("Expected single"),
        };
        assert_eq!(
            compressed_props.get("format").unwrap().as_str(),
            Some("gzip")
        );
        assert!(compressed_bytes.len() < original.len()); // Should be smaller for repeated data

        // Decompress
        let decompressed = GzipDecompress
            .convert(&compressed_bytes, &compressed_props)
            .unwrap();
        let (decompressed_bytes, _) = match decompressed {
            ConvertOutput::Single(b, p) => (b, p),
            _ => panic!("Expected single"),
        };
        assert_eq!(decompressed_bytes, original);
    }

    #[test]
    #[cfg(feature = "zstd")]
    fn test_zstd_roundtrip() {
        use crate::{ZstdCompress, ZstdDecompress};

        let original = b"Hello, World! This is some test data that should compress well.";
        let props = Properties::new().with("format", "raw");

        // Compress
        let compressed = ZstdCompress.convert(original, &props).unwrap();
        let (compressed_bytes, _) = match compressed {
            ConvertOutput::Single(b, p) => (b, p),
            _ => panic!("Expected single"),
        };

        // Decompress
        let decompressed = ZstdDecompress
            .convert(&compressed_bytes, &Properties::new())
            .unwrap();
        let (decompressed_bytes, _) = match decompressed {
            ConvertOutput::Single(b, p) => (b, p),
            _ => panic!("Expected single"),
        };
        assert_eq!(decompressed_bytes, original);
    }

    #[test]
    #[cfg(feature = "ini")]
    fn test_ini_to_json() {
        use crate::IniToJson;

        let input = b"[section]\nkey=value\nnum=42\n";
        let props = Properties::new().with("format", "ini");

        let result = IniToJson.convert(input, &props).unwrap();
        let (output, out_props) = match result {
            ConvertOutput::Single(b, p) => (b, p),
            _ => panic!("Expected single"),
        };

        let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(out_props.get("format").unwrap().as_str(), Some("json"));
        assert_eq!(value["section"]["key"], "value");
        assert_eq!(value["section"]["num"], "42");
    }

    #[test]
    #[cfg(feature = "markdown")]
    fn test_markdown_to_html() {
        use crate::MarkdownToHtml;

        let input = b"# Hello\n\nThis is **bold** text.";
        let props = Properties::new().with("format", "markdown");

        let result = MarkdownToHtml.convert(input, &props).unwrap();
        let (output, out_props) = match result {
            ConvertOutput::Single(b, p) => (b, p),
            _ => panic!("Expected single"),
        };

        let output_str = String::from_utf8(output).unwrap();
        assert_eq!(out_props.get("format").unwrap().as_str(), Some("html"));
        assert!(output_str.contains("<h1>Hello</h1>"));
        assert!(output_str.contains("<strong>bold</strong>"));
    }

    #[test]
    #[cfg(feature = "html2text")]
    fn test_html_to_text() {
        use crate::HtmlToText;

        let input = b"<html><body><h1>Title</h1><p>Hello, <b>World</b>!</p></body></html>";
        let props = Properties::new().with("format", "html");

        let result = HtmlToText.convert(input, &props).unwrap();
        let (output, out_props) = match result {
            ConvertOutput::Single(b, p) => (b, p),
            _ => panic!("Expected single"),
        };

        let output_str = String::from_utf8(output).unwrap();
        assert_eq!(out_props.get("format").unwrap().as_str(), Some("text"));
        assert!(output_str.contains("Title"));
        assert!(output_str.contains("Hello"));
        assert!(output_str.contains("World"));
    }
}
