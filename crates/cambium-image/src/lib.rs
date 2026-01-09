//! Image format converters for Cambium.
//!
//! This crate provides converters between various image formats
//! using the `image` crate. Enable formats via feature flags.
//!
//! # Features
//!
//! ## Lossless formats
//! - `png` (default) - Portable Network Graphics
//! - `gif` (default) - Graphics Interchange Format
//! - `bmp` - Windows Bitmap
//! - `ico` - Windows Icon
//! - `tiff` - Tagged Image File Format
//! - `tga` - Truevision TGA
//! - `pnm` - Portable Any Map (PBM, PGM, PPM, PAM)
//! - `farbfeld` - Farbfeld image format
//! - `qoi` - Quite OK Image format
//!
//! ## Lossy formats
//! - `jpeg` (default) - JPEG
//! - `webp` (default) - WebP
//! - `avif` - AV1 Image File Format
//!
//! ## HDR formats
//! - `openexr` - OpenEXR high dynamic range
//! - `hdr` - Radiance HDR
//!
//! ## Feature group
//! - `all` - All image formats

use cambium::{
    ConvertError, ConvertOutput, Converter, ConverterDecl, Properties, PropertyPattern, Registry,
};
use image::{DynamicImage, ImageFormat};
use std::io::Cursor;

/// Register all enabled image converters with the registry.
pub fn register_all(registry: &mut Registry) {
    let formats = enabled_formats();

    // Register converters between all pairs of enabled formats
    for (from_name, from_fmt) in &formats {
        for (to_name, to_fmt) in &formats {
            if from_name != to_name {
                registry.register(ImageConverter::new(from_name, *from_fmt, to_name, *to_fmt));
            }
        }
    }
}

/// Get list of enabled formats based on feature flags.
/// Returns (format_name, ImageFormat) pairs.
pub fn enabled_formats() -> Vec<(&'static str, ImageFormat)> {
    vec![
        #[cfg(feature = "png")]
        ("png", ImageFormat::Png),
        #[cfg(feature = "jpeg")]
        ("jpg", ImageFormat::Jpeg),
        #[cfg(feature = "webp")]
        ("webp", ImageFormat::WebP),
        #[cfg(feature = "gif")]
        ("gif", ImageFormat::Gif),
        #[cfg(feature = "bmp")]
        ("bmp", ImageFormat::Bmp),
        #[cfg(feature = "ico")]
        ("ico", ImageFormat::Ico),
        #[cfg(feature = "tiff")]
        ("tiff", ImageFormat::Tiff),
        #[cfg(feature = "tga")]
        ("tga", ImageFormat::Tga),
        #[cfg(feature = "pnm")]
        ("pnm", ImageFormat::Pnm),
        #[cfg(feature = "farbfeld")]
        ("farbfeld", ImageFormat::Farbfeld),
        #[cfg(feature = "qoi")]
        ("qoi", ImageFormat::Qoi),
        #[cfg(feature = "avif")]
        ("avif", ImageFormat::Avif),
        #[cfg(feature = "openexr")]
        ("exr", ImageFormat::OpenExr),
        #[cfg(feature = "hdr")]
        ("hdr", ImageFormat::Hdr),
    ]
}

/// A converter between two image formats.
pub struct ImageConverter {
    decl: ConverterDecl,
    from_format: ImageFormat,
    to_format: ImageFormat,
    to_name: &'static str,
}

impl ImageConverter {
    pub fn new(
        from_name: &'static str,
        from_format: ImageFormat,
        to_name: &'static str,
        to_format: ImageFormat,
    ) -> Self {
        let id = format!("image.{}-to-{}", from_name, to_name);
        let decl = ConverterDecl::simple(
            &id,
            PropertyPattern::new().eq("format", from_name),
            PropertyPattern::new().eq("format", to_name),
        )
        .description(format!(
            "Convert {} to {} via image crate",
            from_name.to_uppercase(),
            to_name.to_uppercase()
        ));

        Self {
            decl,
            from_format,
            to_format,
            to_name,
        }
    }
}

impl Converter for ImageConverter {
    fn decl(&self) -> &ConverterDecl {
        &self.decl
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        // Decode image
        let img = image::load_from_memory_with_format(input, self.from_format)
            .map_err(|e| ConvertError::InvalidInput(format!("Failed to decode image: {}", e)))?;

        // Encode to target format
        let output = encode_image(&img, self.to_format)?;

        // Build output properties
        let mut out_props = props.clone();
        out_props.insert("format".into(), self.to_name.into());

        // Add image metadata
        out_props.insert("width".into(), (img.width() as i64).into());
        out_props.insert("height".into(), (img.height() as i64).into());

        Ok(ConvertOutput::Single(output, out_props))
    }
}

/// Encode a DynamicImage to bytes in the specified format.
fn encode_image(img: &DynamicImage, format: ImageFormat) -> Result<Vec<u8>, ConvertError> {
    let mut buf = Cursor::new(Vec::new());

    img.write_to(&mut buf, format)
        .map_err(|e| ConvertError::Failed(format!("Failed to encode image: {}", e)))?;

    Ok(buf.into_inner())
}

/// Detect image format from file extension.
pub fn detect_format(path: &str) -> Option<(&'static str, ImageFormat)> {
    let ext = path.rsplit('.').next()?;
    match ext.to_lowercase().as_str() {
        "png" => Some(("png", ImageFormat::Png)),
        "jpg" | "jpeg" => Some(("jpg", ImageFormat::Jpeg)),
        "webp" => Some(("webp", ImageFormat::WebP)),
        "gif" => Some(("gif", ImageFormat::Gif)),
        "bmp" => Some(("bmp", ImageFormat::Bmp)),
        "ico" => Some(("ico", ImageFormat::Ico)),
        "tif" | "tiff" => Some(("tiff", ImageFormat::Tiff)),
        "tga" => Some(("tga", ImageFormat::Tga)),
        "pnm" | "pbm" | "pgm" | "ppm" | "pam" => Some(("pnm", ImageFormat::Pnm)),
        "ff" | "farbfeld" => Some(("farbfeld", ImageFormat::Farbfeld)),
        "qoi" => Some(("qoi", ImageFormat::Qoi)),
        "avif" => Some(("avif", ImageFormat::Avif)),
        "exr" => Some(("exr", ImageFormat::OpenExr)),
        "hdr" => Some(("hdr", ImageFormat::Hdr)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cambium::PropertiesExt;

    #[test]
    fn test_register_all() {
        let mut registry = Registry::new();
        register_all(&mut registry);

        // Should have n*(n-1) converters for n formats
        let n = enabled_formats().len();
        assert_eq!(registry.len(), n * (n - 1));
    }

    #[test]
    #[cfg(all(feature = "png", feature = "jpeg"))]
    fn test_png_to_jpeg() {
        // Create a minimal 1x1 PNG
        let png_data = create_test_png();

        let converter = ImageConverter::new("png", ImageFormat::Png, "jpg", ImageFormat::Jpeg);
        let props = Properties::new().with("format", "png");

        let result = converter.convert(&png_data, &props).unwrap();

        match result {
            ConvertOutput::Single(output, out_props) => {
                // JPEG magic bytes: 0xFF 0xD8 0xFF
                assert!(output.starts_with(&[0xFF, 0xD8, 0xFF]));
                assert_eq!(out_props.get("format").unwrap().as_str(), Some("jpg"));
                assert_eq!(out_props.get("width").unwrap().as_i64(), Some(1));
                assert_eq!(out_props.get("height").unwrap().as_i64(), Some(1));
            }
            _ => panic!("Expected single output"),
        }
    }

    #[cfg(feature = "png")]
    fn create_test_png() -> Vec<u8> {
        use image::{ImageBuffer, Rgba};

        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(1, 1, Rgba([255, 0, 0, 255]));
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    #[test]
    #[cfg(all(feature = "png", feature = "webp"))]
    fn test_png_to_webp() {
        let png_data = create_test_png();

        let converter = ImageConverter::new("png", ImageFormat::Png, "webp", ImageFormat::WebP);
        let props = Properties::new().with("format", "png");

        let result = converter.convert(&png_data, &props).unwrap();

        match result {
            ConvertOutput::Single(output, out_props) => {
                // WebP magic: "RIFF" ... "WEBP"
                assert!(output.starts_with(b"RIFF"));
                assert_eq!(out_props.get("format").unwrap().as_str(), Some("webp"));
            }
            _ => panic!("Expected single output"),
        }
    }
}
