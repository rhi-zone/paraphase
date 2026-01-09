//! Serde-based format converters for Cambium.
//!
//! This crate provides converters between various data serialization formats
//! using the serde ecosystem. Enable formats via feature flags.
//!
//! # Features
//!
//! - `json` (default) - JSON via serde_json
//! - `yaml` (default) - YAML via serde_yaml
//! - `toml` (default) - TOML via toml
//! - `ron` - Rusty Object Notation
//! - `msgpack` - MessagePack binary format
//! - `cbor` - CBOR binary format
//! - `csv` - CSV (limited to arrays of flat objects)
//! - `full` - All formats

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
}

/// Get list of enabled formats based on feature flags.
pub fn enabled_formats() -> Vec<&'static str> {
    // CSV is special - only works with arrays of flat objects
    // Don't include in general conversion matrix
    [
        #[cfg(feature = "json")]
        "json",
        #[cfg(feature = "yaml")]
        "yaml",
        #[cfg(feature = "toml")]
        "toml",
        #[cfg(feature = "ron")]
        "ron",
        #[cfg(feature = "msgpack")]
        "msgpack",
        #[cfg(feature = "cbor")]
        "cbor",
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

/// Deserialize bytes to a serde Value.
fn deserialize(format: &str, data: &[u8]) -> Result<serde_json::Value, ConvertError> {
    match format {
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

        #[cfg(feature = "msgpack")]
        "msgpack" => rmp_serde::from_slice(data)
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid MessagePack: {}", e))),

        #[cfg(feature = "cbor")]
        "cbor" => ciborium::from_reader(data)
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid CBOR: {}", e))),

        _ => Err(ConvertError::Failed(format!(
            "Unsupported source format: {}",
            format
        ))),
    }
}

/// Serialize a serde Value to bytes.
fn serialize(format: &str, value: &serde_json::Value) -> Result<Vec<u8>, ConvertError> {
    match format {
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

        // Should have n*(n-1) converters for n formats
        let n = enabled_formats().len();
        assert_eq!(registry.len(), n * (n - 1));
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
}
