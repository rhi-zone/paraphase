//! PKI/certificate format converters for Paraphase.
//!
//! Provides PEM ↔ DER conversion using the pem-rfc7468 crate.
//!
//! # Features
//! - `pem` (default) — PEM encode/decode via pem-rfc7468

use paraphase_core::{
    ConvertError, ConvertOutput, Converter, ConverterDecl, Properties, PropertyPattern, Registry,
};

/// Register all enabled PKI converters with the registry.
pub fn register_all(registry: &mut Registry) {
    #[cfg(feature = "pem")]
    {
        registry.register(PemToDer);
        registry.register(DerToPem);
    }
}

// ============================================
// PEM ↔ DER
// ============================================

#[cfg(feature = "pem")]
mod pem_impl {
    use super::*;

    /// Decode PEM-encoded data to raw DER bytes.
    ///
    /// Input: PEM text with `-----BEGIN {label}-----` header.
    /// Output: raw DER bytes, `format = "der"`, `pem_label = "{label}"`.
    pub struct PemToDer;

    impl Converter for PemToDer {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "pki.pem-to-der",
                    PropertyPattern::new().eq("format", "pem"),
                    PropertyPattern::new().eq("format", "der"),
                )
                .description("Decode PEM to raw DER bytes")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let (label, der_data) = pem_rfc7468::decode_vec(input)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid PEM: {}", e)))?;

            let mut out_props = props.clone();
            out_props.insert("format".into(), "der".into());
            out_props.insert("pem_label".into(), label.to_string().into());
            Ok(ConvertOutput::Single(der_data, out_props))
        }
    }

    /// Encode raw DER bytes as PEM.
    ///
    /// Input: DER bytes, `format = "der"`.
    /// Props: optional `pem_label` (default: "CERTIFICATE").
    /// Output: PEM text, `format = "pem"`.
    pub struct DerToPem;

    impl Converter for DerToPem {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "pki.der-to-pem",
                    PropertyPattern::new().eq("format", "der"),
                    PropertyPattern::new().eq("format", "pem"),
                )
                .description(
                    "Encode DER bytes as PEM (requires pem_label property, default: CERTIFICATE)",
                )
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let label = props
                .get("pem_label")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "CERTIFICATE".to_string());

            let pem = pem_rfc7468::encode_string(&label, pem_rfc7468::LineEnding::LF, input)
                .map_err(|e| ConvertError::Failed(format!("PEM encode failed: {}", e)))?;

            let mut out_props = props.clone();
            out_props.insert("format".into(), "pem".into());
            Ok(ConvertOutput::Single(pem.into_bytes(), out_props))
        }
    }
}

#[cfg(feature = "pem")]
pub use pem_impl::{DerToPem, PemToDer};

#[cfg(test)]
mod tests {
    use super::*;
    use paraphase_core::PropertiesExt;

    #[test]
    #[cfg(feature = "pem")]
    fn test_pem_roundtrip() {
        // A minimal DER-encoded structure (just some bytes for testing)
        let der_data = b"\x30\x0a\x02\x01\x01\x02\x01\x02\x02\x01\x03";

        // DER → PEM
        let props = Properties::new().with("format", "der");
        let result = DerToPem.convert(der_data, &props).unwrap();
        let (pem_bytes, pem_props) = match result {
            ConvertOutput::Single(b, p) => (b, p),
            _ => panic!("Expected single"),
        };
        assert_eq!(
            pem_props.get("format").and_then(|v| v.as_str()),
            Some("pem")
        );
        let pem_text = std::str::from_utf8(&pem_bytes).unwrap();
        assert!(pem_text.contains("-----BEGIN CERTIFICATE-----"));
        assert!(pem_text.contains("-----END CERTIFICATE-----"));

        // PEM → DER
        let props2 = Properties::new().with("format", "pem");
        let result2 = PemToDer.convert(&pem_bytes, &props2).unwrap();
        let (der_out, der_props) = match result2 {
            ConvertOutput::Single(b, p) => (b, p),
            _ => panic!("Expected single"),
        };
        assert_eq!(
            der_props.get("format").and_then(|v| v.as_str()),
            Some("der")
        );
        assert_eq!(
            der_props.get("pem_label").and_then(|v| v.as_str()),
            Some("CERTIFICATE")
        );
        assert_eq!(der_out, der_data);
    }
}
