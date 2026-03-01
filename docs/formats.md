# Formats Reference

Complete list of formats supported by Paraphase converters.

## Serde Formats (paraphase-serde)

All serde formats use `serde_json::Value` as an intermediate representation, enabling conversion between any pair of enabled formats.

### Text Formats

| Format | Feature | Extensions | Notes |
|--------|---------|------------|-------|
| JSON | `json` | .json | Default enabled |
| YAML | `yaml` | .yaml, .yml | Default enabled |
| TOML | `toml` | .toml | Default enabled |
| RON | `ron` | .ron | Rust Object Notation |
| JSON5 | `json5` | .json5 | JSON with comments, trailing commas |
| Hjson | `hjson` | .hjson | Human JSON; comments + unquoted strings |
| KDL | `kdl` | .kdl | KDL Document Language |
| HCL | `hcl` | .hcl, .tf | HashiCorp Configuration Language |
| XML | `xml` | .xml | Via quick-xml |
| TSV | `tsv` | .tsv | Tab-separated values |
| INI | `ini` | .ini | Key-value config |
| Java properties | `properties` | .properties | Java-style key=value |
| .env | `dotenv` | .env | Environment variable files |
| S-expressions | `lexpr` | .lisp, .sexp | Lisp-style |
| URL-encoded | `urlencoded` | - | Form data |
| Query strings | `qs` | - | Nested query params |

### Planned: amazon-ion + paraphase-ion

Amazon Ion (JSON superset from AWS) — standalone `amazon-ion` crate (useful beyond Paraphase),
roll our own from the public spec. Binary + text, ~3,500 LOC, well-specified. Not waiting for
`ion-rs` (unstable API, ~1yr of unreleased commits). `paraphase-ion` wraps `amazon-ion` into
the converter registry. Rescribe depends on `amazon-ion` for KFX support.

### Binary Formats

| Format | Feature | Extensions | Notes |
|--------|---------|------------|-------|
| MessagePack | `msgpack` | .msgpack, .mp | Compact binary JSON-like |
| CBOR | `cbor` | .cbor | Concise Binary Object Representation |
| Bincode | `bincode` | .bincode, .bc | Rust-native binary |
| Postcard | `postcard` | .postcard, .pc | Embedded-friendly |
| BSON | `bson` | .bson | MongoDB binary format |
| FlexBuffers | `flexbuffers` | .flexbuf | Schema-less FlatBuffers |
| Bencode | `bencode` | .bencode, .torrent | BitTorrent format |
| Pickle | `pickle` | .pickle, .pkl | Python serialization |
| Property List | `plist` | .plist | Apple binary plist |

### Feature Groups

```toml
# Cargo.toml for paraphase-serde
[features]
default = ["json", "yaml", "toml"]
all = ["json", "yaml", "toml", "ron", "json5", "hjson", "kdl", "hcl",
       "xml", "tsv", "ini", "properties", "dotenv", "lexpr",
       "urlencoded", "qs", "msgpack", "cbor", "bincode", "postcard",
       "bson", "flexbuffers", "bencode", "pickle", "plist"]
```

## Image Formats (paraphase-image)

All image formats use `image::DynamicImage` as an intermediate representation.

### Lossless Formats

| Format | Feature | Extensions | Notes |
|--------|---------|------------|-------|
| PNG | `png` | .png | Default enabled |
| GIF | `gif` | .gif | Default enabled, animated support |
| BMP | `bmp` | .bmp | Windows bitmap |
| ICO | `ico` | .ico | Windows icon |
| TIFF | `tiff` | .tif, .tiff | Tagged image |
| TGA | `tga` | .tga | Truevision |
| PNM | `pnm` | .pnm, .pbm, .pgm, .ppm, .pam | Portable anymap family |
| Farbfeld | `farbfeld` | .ff | Simple lossless |
| QOI | `qoi` | .qoi | Quite OK Image |

### Lossy Formats

| Format | Feature | Extensions | Notes |
|--------|---------|------------|-------|
| JPEG | `jpeg` | .jpg, .jpeg | Default enabled |
| WebP | `webp` | .webp | Default enabled |
| AVIF | `avif` | .avif | AV1-based |

### HDR Formats

| Format | Feature | Extensions | Notes |
|--------|---------|------------|-------|
| OpenEXR | `openexr` | .exr | High dynamic range |
| Radiance HDR | `hdr` | .hdr | RGBE format |

### Feature Groups

```toml
# Cargo.toml for paraphase-image
[features]
default = ["png", "jpeg", "webp", "gif"]
all = ["png", "jpeg", "webp", "gif", "bmp", "ico", "tiff", "tga",
       "pnm", "farbfeld", "qoi", "avif", "openexr", "hdr"]
```

### Image Transforms

Beyond format conversion, paraphase-image provides transform operations:

| Converter | Description | Options |
|-----------|-------------|---------|
| `image.resize` | Resize image | `max_width`, `max_height`, `scale`, `target_width`, `target_height` |
| `image.crop-aspect` | Crop to aspect ratio | `aspect` (e.g., "16:9"), `gravity` |
| `image.watermark` | Overlay watermark | `position`, `opacity`, `margin` (multi-input) |

**Resize options:**

- `max_width` / `max_height`: Fit within bounds, preserving aspect ratio (no upscaling)
- `scale`: Scale factor (e.g., 0.5 for half size)
- `target_width` / `target_height`: Exact dimensions (may change aspect ratio)

**Gravity presets** (for crop anchor point):

| Preset | Aliases |
|--------|---------|
| `top-left` | `nw`, `northwest` |
| `top` | `n`, `north` |
| `top-right` | `ne`, `northeast` |
| `left` | `w`, `west` |
| `center` | `c`, `middle` (default) |
| `right` | `e`, `east` |
| `bottom-left` | `sw`, `southwest` |
| `bottom` | `s`, `south` |
| `bottom-right` | `se`, `southeast` |

**CLI usage:**

```bash
# Resize to fit within 1024px width
paraphase convert photo.png photo.webp --max-width 1024

# Scale to 50%
paraphase convert photo.png thumb.png --scale 0.5

# Crop to 16:9, keeping top of image
paraphase convert photo.png banner.png --aspect 16:9 --gravity top

# Combine: crop to square, resize, convert format
paraphase convert photo.png avatar.webp --aspect 1:1 --max-width 200

# Add watermark
paraphase convert photo.png branded.png --watermark logo.png

# Watermark with options
paraphase convert photo.png branded.png --watermark logo.png \
  --watermark-position bottom-right --watermark-opacity 0.5 --watermark-margin 20
```

**Watermark options:**

- `position`: Where to place the watermark (uses gravity presets above)
- `opacity`: Watermark transparency (0.0-1.0, default 1.0)
- `margin`: Pixels from edge (default 0)

## CSV / XLSX (paraphase-serde)

CSV and XLSX write are handled by `paraphase-serde` with dedicated features.

| Converter | ID | Feature | Description |
|-----------|-----|---------|-------------|
| CSV → JSON | `serde.csv-to-json` | `csv` | Parse CSV to JSON array of objects (first row = headers) |
| JSON → CSV | `serde.json-to-csv` | `csv` | Serialize JSON array of flat objects to CSV |
| JSON → XLSX | `serde.json-to-xlsx` | `xlsxwrite` | Write JSON array of objects to XLSX spreadsheet |

```bash
paraphase convert data.csv data.json
paraphase convert data.json out.csv
paraphase convert data.json out.xlsx
```

## Vector Graphics (paraphase-vector)

SVG rasterization via `resvg`/`tiny-skia`.

| Converter | ID | Feature | Description |
|-----------|-----|---------|-------------|
| SVG → PNG | `vector.svg-to-png` | `svg` | Render SVG to PNG |
| SVG → JPEG | `vector.svg-to-jpeg` | `svg` | Render SVG to JPEG |
| SVG → WebP | `vector.svg-to-webp` | `svg` | Render SVG to WebP |

**Input properties:** optional `width`, `height` to override render resolution.

```bash
paraphase convert logo.svg logo.png
paraphase convert diagram.svg diagram.jpg
```

## Font Formats (paraphase-font)

TTF/OTF ↔ WOFF1 conversion in pure Rust using flate2.

| Converter | ID | Feature | Description |
|-----------|-----|---------|-------------|
| TTF → WOFF | `font.ttf-to-woff` | `woff` | Wrap TTF in WOFF1 container |
| OTF → WOFF | `font.otf-to-woff` | `woff` | Wrap OTF in WOFF1 container |
| WOFF → TTF | `font.woff-to-ttf` | `woff` | Extract TTF from WOFF1 container |

```bash
paraphase convert font.ttf font.woff
paraphase convert font.otf font.woff
paraphase convert font.woff font.ttf
```

## Geospatial Formats (paraphase-geo)

GPX ↔ GeoJSON via the `gpx` crate.

| Converter | ID | Feature | Description |
|-----------|-----|---------|-------------|
| GPX → GeoJSON | `geo.gpx-to-geojson` | `gpx` | Convert GPX tracks/waypoints to GeoJSON FeatureCollection |
| GeoJSON → GPX | `geo.geojson-to-gpx` | `gpx` | Convert GeoJSON to GPX |

```bash
paraphase convert tracks.gpx tracks.geojson
paraphase convert route.geojson route.gpx
```

## PKI / Certificate Formats (paraphase-pki)

PEM ↔ DER via `pem-rfc7468`.

| Converter | ID | Feature | Description |
|-----------|-----|---------|-------------|
| PEM → DER | `pki.pem-to-der` | `pem` | Decode PEM to raw DER bytes |
| DER → PEM | `pki.der-to-pem` | `pem` | Encode DER bytes as PEM |

**Properties:** `pem_label` — the PEM label string (e.g., "CERTIFICATE", "PRIVATE KEY"). Preserved on decode, required on encode (default: "CERTIFICATE").

```bash
paraphase convert cert.pem cert.der
paraphase convert cert.der cert.pem
```

## Subtitle Formats (paraphase-subtitle)

Pure Rust SRT/VTT/SBV conversion — no external dependencies.

| Converter | ID | Feature | Description |
|-----------|-----|---------|-------------|
| SRT → VTT | `subtitle.srt-to-vtt` | `srt` + `vtt` | SubRip to WebVTT |
| VTT → SRT | `subtitle.vtt-to-srt` | `srt` + `vtt` | WebVTT to SubRip |
| SRT → SBV | `subtitle.srt-to-sbv` | `srt` + `sbv` | SubRip to SubViewer |
| SBV → SRT | `subtitle.sbv-to-srt` | `srt` + `sbv` | SubViewer to SubRip |
| VTT → SBV | `subtitle.vtt-to-sbv` | `vtt` + `sbv` | WebVTT to SubViewer |
| SBV → VTT | `subtitle.sbv-to-vtt` | `vtt` + `sbv` | SubViewer to WebVTT |

```bash
paraphase convert subtitles.srt subtitles.vtt
paraphase convert subtitles.vtt subtitles.srt
paraphase convert subtitles.sbv subtitles.vtt
```

## Color Palette Formats (paraphase-color)

Pure Rust GPL/ACO/ASE ↔ JSON conversion — no external dependencies.

| Converter | ID | Feature | Description |
|-----------|-----|---------|-------------|
| GPL → JSON | `color.gpl-to-json` | `gpl` | Parse GIMP Palette to JSON |
| JSON → GPL | `color.json-to-gpl` | `gpl` | Serialize JSON palette to GIMP Palette |
| ACO → JSON | `color.aco-to-json` | `aco` | Parse Photoshop Color Swatches to JSON |
| JSON → ACO | `color.json-to-aco` | `aco` | Serialize JSON palette to ACO |
| ASE → JSON | `color.ase-to-json` | `ase` | Parse Adobe Swatch Exchange to JSON |
| JSON → ASE | `color.json-to-ase` | `ase` | Serialize JSON palette to ASE |

**JSON palette format:**
```json
{
  "name": "My Palette",
  "colors": [
    { "r": 255, "g": 0, "b": 0, "name": "Red" },
    { "r": 0, "g": 128, "b": 0, "name": "Green" }
  ]
}
```

```bash
paraphase convert palette.gpl palette.json
paraphase convert palette.json palette.gpl
paraphase convert swatches.aco swatches.json
paraphase convert palette.json palette.ase
```

## Audio Formats (paraphase-audio)

Pure Rust audio processing via Symphonia (decode) and Hound (WAV encode).

### Supported Formats

| Format | Decode | Encode | Feature |
|--------|--------|--------|---------|
| WAV | ✓ | ✓ | `wav` |
| FLAC | ✓ | - | `flac` |
| MP3 | ✓ | - | `mp3` |
| OGG Vorbis | ✓ | - | `ogg` |
| AAC | ✓ | - | `aac` |

**Note:** Currently all formats decode to WAV. Encoders for other formats are planned.

### Feature Groups

```toml
# Cargo.toml for paraphase-audio
[features]
default = ["wav", "flac", "mp3", "ogg"]
all = ["wav", "flac", "mp3", "ogg", "aac"]
```

**CLI usage:**

```bash
# Convert MP3 to WAV
paraphase convert song.mp3 song.wav

# Convert FLAC to WAV
paraphase convert album.flac album.wav

# Convert OGG to WAV
paraphase convert audio.ogg audio.wav
```

## Video Formats (paraphase-video)

All video formats use FFmpeg as the transcoding backend. **Requires FFmpeg installed at runtime.**

### Container Formats

| Format | Feature | Extensions | Default Codecs |
|--------|---------|------------|----------------|
| MP4 | `mp4` | .mp4, .m4v | H.264 + AAC |
| WebM | `webm` | .webm | VP9 + Opus |
| MKV | `mkv` | .mkv | H.264 + AAC |
| AVI | `avi` | .avi | MPEG-4 + MP3 |
| MOV | `mov` | .mov, .qt | H.264 + AAC |
| GIF | `gif` | .gif | GIF (animated) |

### Feature Groups

```toml
# Cargo.toml for paraphase-video
[features]
default = ["mp4", "webm", "gif"]
all = ["mp4", "webm", "mkv", "avi", "mov", "gif", "audio"]
```

### Video Transforms

| Converter | Description | Options |
|-----------|-------------|---------|
| `video.resize` | Resize video | `max_width`, `max_height`, `scale` |

### Quality Presets

| Preset | CRF | Use Case |
|--------|-----|----------|
| `low` | 28 | Smaller file size |
| `medium` | 23 | Balanced (default) |
| `high` | 18 | Higher quality |
| `lossless` | 0 | No quality loss |

**CLI usage:**

```bash
# Convert MP4 to WebM
paraphase convert video.mp4 video.webm

# Convert with quality preset
paraphase convert video.mp4 video.webm --quality high

# Resize video
paraphase convert video.mp4 small.mp4 --max-width 720

# GIF to video
paraphase convert animation.gif video.mp4
```

## CLI Feature Flags

The CLI combines all converter backends:

```toml
# Cargo.toml for paraphase-cli
[features]
default = ["serde", "image"]

# Include backends
serde = ["dep:paraphase-serde"]
image = ["dep:paraphase-image"]
video = ["dep:paraphase-video"]  # Requires FFmpeg
audio = ["dep:paraphase-audio"]

# Enable all formats per backend
serde-all = ["serde", "paraphase-serde/all"]
image-all = ["image", "paraphase-image/all"]
video-all = ["video", "paraphase-video/all"]
audio-all = ["audio", "paraphase-audio/all"]

# Everything (video excluded from default, requires FFmpeg)
all = ["serde-all", "image-all", "video-all", "audio-all"]
```

### Installation Examples

```bash
# Default: common serde + common image formats
cargo install paraphase-cli

# All formats
cargo install paraphase-cli --features all

# Only serde formats (no image support)
cargo install paraphase-cli --no-default-features --features serde-all

# Only image formats (no serde support)
cargo install paraphase-cli --no-default-features --features image-all

# Specific formats only
cargo install paraphase-cli --no-default-features \
  --features paraphase-serde/json,paraphase-serde/yaml,paraphase-image/png
```

## Converter Naming

Converters follow the pattern `{crate}.{from}-to-{to}`:

- `serde.json-to-yaml`
- `serde.toml-to-msgpack`
- `image.png-to-webp`
- `image.jpg-to-gif`

List all available converters:

```bash
paraphase list
```

## Adding Custom Converters

Implement the `Converter` trait:

```rust
use paraphase::{Converter, ConverterDecl, ConvertError, ConvertOutput, Properties, PropertyPattern};

pub struct MyConverter {
    decl: ConverterDecl,
}

impl MyConverter {
    pub fn new() -> Self {
        let decl = ConverterDecl::simple(
            "my.foo-to-bar",
            PropertyPattern::new().eq("format", "foo"),
            PropertyPattern::new().eq("format", "bar"),
        ).description("Convert foo to bar");

        Self { decl }
    }
}

impl Converter for MyConverter {
    fn decl(&self) -> &ConverterDecl {
        &self.decl
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        // Transform input bytes to output bytes
        let output = transform(input)?;

        let mut out_props = props.clone();
        out_props.insert("format".into(), "bar".into());

        Ok(ConvertOutput::Single(output, out_props))
    }
}
```

Register with a registry:

```rust
let mut registry = Registry::new();
registry.register(MyConverter::new());
```
