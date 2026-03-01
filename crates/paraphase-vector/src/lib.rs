//! Vector graphics converters for Paraphase.
//!
//! Rasterizes SVG to PNG, JPEG, or WebP using resvg/tiny-skia.
//!
//! # Features
//! - `svg` (default) — SVG → PNG/JPEG/WebP via resvg

use paraphase_core::{
    ConvertError, ConvertOutput, Converter, ConverterDecl, Properties, PropertyPattern, Registry,
};

/// Register all enabled vector converters with the registry.
pub fn register_all(registry: &mut Registry) {
    #[cfg(feature = "svg")]
    {
        registry.register(SvgToPng);
        registry.register(SvgToJpeg);
        registry.register(SvgToWebp);
    }
}

// ============================================
// SVG rasterization
// ============================================

#[cfg(feature = "svg")]
mod svg_impl {
    use super::*;
    use image::{ImageFormat, RgbaImage};
    use std::io::Cursor;

    /// Render SVG bytes to a tiny_skia Pixmap at the given (optional) dimensions.
    fn render_svg(
        input: &[u8],
        width: Option<u32>,
        height: Option<u32>,
    ) -> Result<(tiny_skia::Pixmap, u32, u32), ConvertError> {
        let options = resvg::usvg::Options::default();
        let tree = resvg::usvg::Tree::from_data(input, &options)
            .map_err(|e| ConvertError::InvalidInput(format!("Invalid SVG: {}", e)))?;

        let svg_size = tree.size();
        let svg_w = svg_size.width();
        let svg_h = svg_size.height();

        let (target_w, target_h) = match (width, height) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => {
                let scale = w as f32 / svg_w;
                (w, ((svg_h * scale).round() as u32).max(1))
            }
            (None, Some(h)) => {
                let scale = h as f32 / svg_h;
                (((svg_w * scale).round() as u32).max(1), h)
            }
            (None, None) => {
                let size = tree.size().to_int_size();
                (size.width().max(1), size.height().max(1))
            }
        };

        let mut pixmap = tiny_skia::Pixmap::new(target_w, target_h)
            .ok_or_else(|| ConvertError::Failed("Failed to create pixmap".into()))?;

        let transform =
            tiny_skia::Transform::from_scale(target_w as f32 / svg_w, target_h as f32 / svg_h);

        resvg::render(&tree, transform, &mut pixmap.as_mut());

        Ok((pixmap, target_w, target_h))
    }

    /// Convert Pixmap to RGBA image buffer.
    fn pixmap_to_rgba(pixmap: &tiny_skia::Pixmap) -> RgbaImage {
        let w = pixmap.width();
        let h = pixmap.height();
        // tiny-skia uses premultiplied RGBA; un-premultiply for standard RGBA
        let data = pixmap.data();
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for chunk in data.chunks(4) {
            let r = chunk[0];
            let g = chunk[1];
            let b = chunk[2];
            let a = chunk[3];
            // Un-premultiply
            if a == 0 {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                rgba.push(((r as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8);
                rgba.push(((g as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8);
                rgba.push(((b as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8);
                rgba.push(a);
            }
        }
        RgbaImage::from_raw(w, h, rgba).unwrap_or_else(|| RgbaImage::new(w, h))
    }

    /// Render SVG to PNG.
    pub struct SvgToPng;

    impl Converter for SvgToPng {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "vector.svg-to-png",
                    PropertyPattern::new().eq("format", "svg"),
                    PropertyPattern::new().eq("format", "png"),
                )
                .description("Render SVG to PNG raster image")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let width = props
                .get("width")
                .and_then(|v| v.as_i64())
                .map(|v| v as u32);
            let height = props
                .get("height")
                .and_then(|v| v.as_i64())
                .map(|v| v as u32);
            let (pixmap, w, h) = render_svg(input, width, height)?;

            let png_data = pixmap
                .encode_png()
                .map_err(|e| ConvertError::Failed(format!("PNG encoding failed: {}", e)))?;

            let mut out_props = props.clone();
            out_props.insert("format".into(), "png".into());
            out_props.insert("width".into(), (w as i64).into());
            out_props.insert("height".into(), (h as i64).into());
            Ok(ConvertOutput::Single(png_data, out_props))
        }
    }

    /// Render SVG to JPEG.
    pub struct SvgToJpeg;

    impl Converter for SvgToJpeg {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "vector.svg-to-jpeg",
                    PropertyPattern::new().eq("format", "svg"),
                    PropertyPattern::new().eq("format", "jpg"),
                )
                .description("Render SVG to JPEG raster image")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let width = props
                .get("width")
                .and_then(|v| v.as_i64())
                .map(|v| v as u32);
            let height = props
                .get("height")
                .and_then(|v| v.as_i64())
                .map(|v| v as u32);
            let (pixmap, w, h) = render_svg(input, width, height)?;

            let rgba = pixmap_to_rgba(&pixmap);
            let rgb = image::DynamicImage::ImageRgba8(rgba).to_rgb8();

            let mut jpeg_data = Vec::new();
            rgb.write_to(&mut Cursor::new(&mut jpeg_data), ImageFormat::Jpeg)
                .map_err(|e| ConvertError::Failed(format!("JPEG encoding failed: {}", e)))?;

            let mut out_props = props.clone();
            out_props.insert("format".into(), "jpg".into());
            out_props.insert("width".into(), (w as i64).into());
            out_props.insert("height".into(), (h as i64).into());
            Ok(ConvertOutput::Single(jpeg_data, out_props))
        }
    }

    /// Render SVG to WebP.
    pub struct SvgToWebp;

    impl Converter for SvgToWebp {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "vector.svg-to-webp",
                    PropertyPattern::new().eq("format", "svg"),
                    PropertyPattern::new().eq("format", "webp"),
                )
                .description("Render SVG to WebP raster image")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let width = props
                .get("width")
                .and_then(|v| v.as_i64())
                .map(|v| v as u32);
            let height = props
                .get("height")
                .and_then(|v| v.as_i64())
                .map(|v| v as u32);
            let (pixmap, w, h) = render_svg(input, width, height)?;

            let rgba = pixmap_to_rgba(&pixmap);
            let dyn_img = image::DynamicImage::ImageRgba8(rgba);

            let mut webp_data = Vec::new();
            dyn_img
                .write_to(&mut Cursor::new(&mut webp_data), ImageFormat::WebP)
                .map_err(|e| ConvertError::Failed(format!("WebP encoding failed: {}", e)))?;

            let mut out_props = props.clone();
            out_props.insert("format".into(), "webp".into());
            out_props.insert("width".into(), (w as i64).into());
            out_props.insert("height".into(), (h as i64).into());
            Ok(ConvertOutput::Single(webp_data, out_props))
        }
    }
}

#[cfg(feature = "svg")]
pub use svg_impl::{SvgToJpeg, SvgToPng, SvgToWebp};
