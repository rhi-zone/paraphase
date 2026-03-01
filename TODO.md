# Paraphase TODO

## Format Conversions (queued simplest → most complex)

### Trivial (pure Rust, minimal code)

- [x] **Base64/Hex encoding** - `base64`, `hex` crates; encode/decode bytes
- [x] **NDJSON/JSON Lines** - split lines + existing serde_json; streaming-friendly

### Simple (pure Rust, self-contained)

- [x] **Compression** - gzip (`flate2`), zstd, brotli; wrap/unwrap bytes
- [x] **INI config** - `rust-ini`; simple key-value config files
- [x] **Character encoding** - `encoding_rs`; UTF-16, Latin-1, Shift-JIS, etc.

### Medium (pure Rust, more logic)

- [x] **Markdown → HTML** - `pulldown-cmark`; CommonMark compliant
- [x] **HTML → text** - `html2text`; strip tags, preserve structure
- [x] **Archives** - `tar`, `zip` crates; extract/create, maps to Multi output

### Complex (schema-based or native deps)

- [x] **Spreadsheets** - `calamine` for XLSX/ODS/XLS reading (read-only)
- [x] **Avro** - `apache-avro`; schema embedded in container files (self-describing)
- [x] **Parquet** - `parquet`; columnar format via Arrow (self-describing)

### Schema-required (need external definition)

These formats require schema files to decode - not "point and shoot":

- [ ] **Protobuf** - `prost`; requires .proto schema files
- [ ] **Cap'n Proto** - `capnp`; zero-copy, requires .capnp schema files

---

## Document Conversion (paraphase-document)

Thin integration with a document IR library (separate project).

See `docs/document-ir-spec.md` for comprehensive spec of the document IR:
- Analysis of Pandoc's strengths/weaknesses
- Property-bag based architecture (aligns with Cambium philosophy)
- Layered representation (semantic, style, layout)
- Fidelity tracking for lossy conversions
- Embedded resource handling

**The document IR is out of Cambium's scope** - it's a standalone library project.

paraphase-document will:
- [ ] Integrate with document IR library (once it exists)
- [ ] Register format converters with Cambium registry
- [ ] Route document conversions through Cambium's executor

## Audio Encoders (paraphase-audio)

Currently only WAV encoding is supported. Adding encoders for other formats:

- [ ] **FLAC encoder** - pure Rust via `flacenc` crate (if stable)
- [ ] **MP3 encoder** - requires `lame` (native dependency)
- [ ] **OGG Vorbis encoder** - requires `libvorbis` (native dependency)
- [ ] **AAC encoder** - requires FFmpeg or native lib
- [ ] **Opus encoder** - consider as modern alternative to OGG

## Video (paraphase-video)

- [ ] Complete frame encoding pipeline (currently scaffold)
- [ ] Audio track passthrough/transcoding
- [ ] Subtitle extraction

## Hand-Rolled Crate Splits

Per philosophy: hand-rolled format implementations belong in standalone crates; `paraphase-*`
is a thin wrapper. Existing crates that need splitting:

- [ ] **paraphase-subtitle** → extract parser/writer to `subtitle-formats` (or similar);
  `paraphase-subtitle` becomes a thin wrapper; see `subparse` crate as prior art —
  covers SSA/ASS/IDX/MicroDVD/VobSub (formats we haven't implemented), non-destructive
  parsing model, and `SubtitleFileInterface` trait design worth referencing
- [ ] **paraphase-color** → extract GPL/ACO/ASE to `palette-formats` (or similar);
  `paraphase-color` becomes a thin wrapper
- [ ] **paraphase-font** → extract WOFF1 impl to `woff` (or similar);
  `paraphase-font` becomes a thin wrapper

New hand-rolled crates start in the right shape from the beginning (`amazon-ion` etc.).

---

## Architecture

See ADR-0006 for the Executor abstraction.

Implemented:
- [x] **SimpleExecutor** - sequential, unbounded memory
- [x] **BoundedExecutor** - sequential with memory limit checking (fail-fast)
- [x] **ParallelExecutor** - rayon + memory budget for batch (requires `parallel` feature)
- [x] **MemoryBudget** - semaphore-like reservation with RAII permits

Future work:
- [ ] **StreamingExecutor** - chunk-based I/O for huge files (requires converter interface changes)

## CLI Usability

Implemented:
- [x] **Shell completions** - `paraphase completions bash/zsh/fish`
- [x] **Man pages** - `paraphase manpage > paraphase.1`
- [x] **Verbose/quiet modes** - `-v` for debug info, `-q` for silent
- [x] **Better format detection** - magic bytes before extension fallback
- [x] **Stdin/stdout piping** - `cat file.mp3 | paraphase convert - -o - --from mp3 --to wav`
- [x] **Batch processing** - `paraphase convert *.mp3 --output-dir out/ --to wav`
- [x] **Progress reporting** - progress bars for batch conversions

Implemented:
- [x] **Presets** - `--preset web` for common conversion profiles
- [x] **Config file** - `~/.config/paraphase/config.toml` for defaults
- [x] **Dynamic presets** - Dew expressions in preset values (requires `dew` feature)

Implemented:
- [x] **Path optimization** - `--optimize quality|speed|size` for multi-path selection
- [x] **Better error messages** - actionable suggestions, format hints, typo detection

## Dynamic Presets (Dew Integration)

With the `dew` feature enabled, preset numeric values can be expressions:

```toml
# ~/.config/paraphase/config.toml
[preset.smart-web]
max_width = "min(width, 1920)"
max_height = "min(height, 1080)"
quality = "if file_size > 5000000 then 70 else 85"

[preset.proportional]
max_width = "width * 0.5"
max_height = "height * 0.5"
```

Available variables (from input file properties):
- `width`, `height` - image dimensions
- `file_size` - input file size in bytes
- Any other numeric property from the input

Expressions use [Dew](https://github.com/rhi-zone/dew) syntax with standard math functions:
- Comparison: `<`, `>`, `<=`, `>=`, `==`, `!=`
- Math: `min`, `max`, `clamp`, `abs`, `sqrt`, `pow`
- Conditionals: `if ... then ... else ...`

Build with expressions: `cargo build -p paraphase-cli --features dew`

## Testing & Quality

Implemented:
- [x] **Integration tests** - 18 end-to-end CLI tests covering:
  - Multi-hop chains (JSON → YAML → TOML, roundtrips)
  - Batch processing with multiple files
  - Progress bar and quiet mode
  - Presets and config
  - Optimize flag variations
- [x] **Unit tests** - Archive roundtrips (tar, zip), format converters
- [x] **CI/CD** - GitHub Actions for check/test/fmt/clippy/doc/build

Implemented:
- [x] **Expansion executor** - `execute_expanding()` properly fans out 1→N through pipeline
- [x] **Aggregation executor** - `execute_aggregating()` for N→1 conversions (files → archive)
- [x] **Compound archives** - `tar.gz`, `tar.zst`, `tgz` with post-aggregation compression
- [x] **Glob support** - `paraphase convert "*.json" --to yaml`
- [x] **Directory recursion** - `-r/--recursive` for tree traversal
- [x] **Batch modes** - `--batch-mode all|per-dir` for different grouping strategies

Known limitations (documented, not bugs):
- Output filenames may collide when processing trees (flat output dir)

Future work:
- [ ] **Benchmarks** - criterion benchmarks for regression tracking
- [ ] **Preserve directory structure** - mirror input tree to output tree

## Complexity Hotspots (threshold >21)
- [ ] `crates/paraphase-cli/src/main.rs:detect_format` (44)
- [ ] `crates/paraphase-audio/src/lib.rs:convert_to_i16` (40)
- [ ] `crates/paraphase-cli/src/main.rs:convert_single_file` (38)
- [ ] `crates/paraphase-image/src/lib.rs:compute_resize_dimensions` (30)
- [ ] `crates/paraphase-cli/src/main.rs:mime_to_format` (29)
- [ ] `crates/paraphase-serde/src/lib.rs:avro_impl.avro_value_to_json` (28)
- [ ] `crates/paraphase-image/src/lib.rs:composite_with_opacity` (27)
- [ ] `crates/paraphase-cli/src/main.rs:cmd_plan_workflow` (21)
- [ ] `crates/paraphase-serde/src/lib.rs:deserialize` (21)
- [ ] `crates/paraphase-serde/src/lib.rs:serialize` (21)

## 3D Formats (paraphase-3d)

### Tier 1 — wrap and ship

- [ ] **STL** - `stl_io`; triangle meshes, common for 3D printing (text + binary variants)
- [ ] **OBJ/Wavefront** - `tobj`; widely supported mesh format (vertices, normals, UVs)
- [ ] **PLY** - `ply-rs`; point clouds and meshes, used in scanning/research
- [ ] **glTF/GLB** - `gltf`; modern "JPEG of 3D", scenes with meshes/materials/animations
- [ ] **3MF** - `quick-xml` + zip; XML-based 3D printing format

### Deferred / no viable path

- [ ] **COLLADA/DAE** - Enormous XML schema; diminishing returns
- [ ] **FBX** - Autodesk proprietary; no viable open-source path

## New Format Priorities

Three tiers based on Rust ecosystem readiness:

- **Tier 1** — production-grade crate exists; wrap and ship
- **Tier 2** — roll our own; compare against existing crates as quality benchmark
- **Tier 3** — complex, deferred

### Tier 1 — Use production-grade crates

| Format(s) | Crate(s) | Notes |
|-----------|----------|-------|
| CSV | `csv` (BurntSushi) | Serde-integrated, extend paraphase-serde |
| PEM ↔ DER | `pem-rfc7468` + `der` (RustCrypto) | Strict RFC 7468; used by rustls |
| SVG → raster | `resvg` + `usvg` | Production-grade, used widely in tooling |
| XLSX write | `rust_xlsxwriter` | Actively maintained; pairs with existing calamine read |
| TTF/OTF ↔ WOFF/WOFF2 | `ttf-parser`, `woff2` | Same author as resvg; WOFF is a compression wrapper |
| GPX | `gpx` | Clean GPS track data; converts naturally to/from GeoJSON |

### Tier 2 — Roll our own (use existing crates as quality benchmark)

Roll our own parsers; audit `subparse`, `srt`, and similar crates for completeness and correctness as a reference bar.

| Format(s) | Existing crates to evaluate | Notes |
|-----------|-----------------------------|-------|
| SRT ↔ VTT | `subparse`, `srt` | Formats are simple enough; existing crates may have edge-case gaps |
| SBV | (none known) | YouTube format, close to SRT |
| ASS/SSA → SRT | `subparse` | Lossy; drops styling. See subparse for prior art |
| IDX/SUB, MicroDVD | `subparse` | DVD/frame-based subtitles; subparse covers these |
| GPL palette | (none) | ~30 lines; no crate justified |
| ACO palette | (none known) | Binary, simple; existing crates unknown/unvetted |
| ASE palette | (none known) | Adobe Swatch Exchange; binary with known structure |

### Tier 3 — Doable but not trivial

These were deferred but are now considered implementable with the current architecture.
See their dedicated sections below for full breakdown.

- **ICS/vCard** — `icalendar` crate unvetted; roll our own for common subset
- **KML** — XML; `quick-xml` handles it; GIS-aware transform needed
- **Shapefile** — `shapefile` crate; multi-file maps to `ConvertOutput::Multi`
- **WKT/WKB** — `wkt` crate from `geo` ecosystem; very small scope
- **JWK** — JSON Web Key; roll-our-own or `jwt-simple`
- **PKCS#12** — `p12` crate (RustCrypto); security-sensitive but bounded API
- **ASS/SSA → SRT** — Pure Rust, lossy; parser ~200 lines
- **TTML/DFXP** — XML subtitles; `quick-xml` + known schema
- **WOFF2** — `woff2` crate; worth re-evaluating for viability
- **3D formats** — STL/OBJ/PLY/glTF/3MF; see dedicated 3D section above
- **CSS color vars** — Simple text parsing; extend paraphase-color

### Genuinely blocked / no viable path

- **EPS** — Requires Ghostscript; no pure-Rust renderer
- **EPUB / MOBI / KFX / AZW3 / FB2** — Deferred; will embed rescribe once it matures
- **FB2** — Deferred pending document IR
- **FBX** — Autodesk proprietary; no viable open-source path
- **COLLADA/DAE** — Enormous XML schema; diminishing returns
- **DXF** — 1000+ page spec; basic meshes possible but surface area is huge
- **ODS write** — `spreadsheet_ods` crate not well-maintained

---

## Subtitles/Captions (paraphase-subtitle)

All subtitle formats are plain text — pure Rust, no native deps.

### Tier 1

- [x] **SRT** - roll our own parser (see New Format Priorities); most common subtitle format
- [x] **VTT** - WebVTT; web standard, near-superset of SRT
- [x] **SBV** - YouTube subtitle format; close to SRT

- [ ] **ASS/SSA → SRT** - lossy (drops styling); `subparse` crate covers this as prior art
- [ ] **IDX/SUB (VobSub)** - DVD subtitle bitmaps; `subparse` covers this as prior art
- [ ] **MicroDVD (.sub)** - frame-based subtitles; `subparse` covers this as prior art
- [ ] **TTML/DFXP** - XML-based; `quick-xml` + known schema; used in broadcast/streaming

Conversion pairs: SRT ↔ VTT is the most requested. ASS → SRT (lossy, drops styling) is common.

---

## Vector/SVG (paraphase-vector)

- [x] **SVG → raster** - `resvg` (Tier 1); render to PNG/JPEG at specified resolution

Complex / deferred:
- [ ] **EPS** - PostScript-based; parsing is hard, likely needs Ghostscript
- [ ] **DXF** - AutoCAD drawing exchange; `dxf` crate exists but format is vast

---

## E-book Formats (paraphase-ebook)

Deferred — will integrate **rescribe** as an embedded backend once it matures.
Formats: EPUB, MOBI, KFX, AZW3, FB2.

Prior art: **boko** (`docs.rs/boko`) — ebook processing engine with IR; covers EPUB/AZW3
read+write, MOBI read. Rescribe may build on or alongside it.

---

## Font Formats (paraphase-font)

- [x] **TTF/OTF ↔ WOFF** - Tier 1; WOFF1 implemented (pure Rust + flate2); WOFF2 deferred
- [ ] **WOFF2** - re-evaluate `woff2` crate viability; if not, `woff2` format is brotli-compressed sfnt

---

## Calendar/Contacts (paraphase-ical)

RFC parsing has edge cases but the common subset is tractable:
- [ ] **ICS/iCal** - RFC 5545; roll our own for VEVENT/VTODO/VALARM; `icalendar` crate as reference
- [ ] **vCard** - RFC 6350; roll our own for common properties; `vcard` crate as reference
- [ ] **jCal** - JSON encoding of iCalendar (RFC 7265); lossless round-trip with ICS
- [ ] **jCard** - JSON encoding of vCard (RFC 7095); lossless round-trip with vCard

---

## GIS/Geospatial (paraphase-geo)

- [x] **GPX** - Tier 1; `gpx` crate; GPS tracks/waypoints, converts naturally to/from GeoJSON

- [ ] **WKT/WKB** - Well-Known Text/Binary; `wkt` crate from `geo` ecosystem; small scope
- [ ] **KML** - Keyhole Markup Language; Google Earth format; XML via `quick-xml`
- [ ] **Shapefile** - `.shp/.dbf/.shx`; `shapefile` crate; multi-file maps to `ConvertOutput::Multi`

---

## Additional Serde Formats (paraphase-serde)

- [ ] **KDL** - `kdl` crate v6; fits the SerdeConverter pattern; config/data language
- [ ] **TSV** - tab-separated values; trivial extension of existing CSV converter
- [ ] **HCL** - HashiCorp Configuration Language; `hcl-rs` crate; Terraform/infra configs
- [ ] **Hjson** - Human JSON; `deser-hjson` crate; previously attempted, needs compat check
- [ ] **Java .properties** - roll our own; simple key=value with `\` escapes
- [ ] **.env/dotenv** - `dotenvy` or roll our own; KEY=value env files

- [ ] **Amazon Ion** - standalone `amazon-ion` crate (useful beyond Paraphase) + thin
  `paraphase-ion` wrapper; roll our own from the public spec (binary + text, ~3.5k LOC);
  rescribe depends on `amazon-ion` for KFX; don't wait for `ion-rs` — spec is well-documented

## Spreadsheet Write Support (paraphase-serde)

- [x] **CSV** - Tier 1; `csv` crate; extend paraphase-serde
- [x] **XLSX write** - Tier 1; `rust_xlsxwriter`; pairs with existing calamine read support
- [ ] **ODS write** - Tier 3; OpenDocument write support; crate situation unclear

---

## Color/Palette Formats (paraphase-color)

- [x] **GPL** - Tier 2 (roll our own); GIMP Palette; plain text, ~30 lines
- [x] **ACO** - Tier 2 (roll our own); Photoshop Color Swatches; binary, simple structure
- [x] **ASE** - Tier 2 (roll our own); Adobe Swatch Exchange; binary with known structure
- [ ] **CSS custom properties** - extract color variables from CSS; simple text parsing, extend paraphase-color

---

## Certificate/Crypto Formats (paraphase-pki)

- [x] **PEM ↔ DER** - Tier 1; `pem-rfc7468` + `der` (RustCrypto); base64 wrapper around DER
- [ ] **PKCS#12/.pfx** - `p12` crate (RustCrypto); certificate+key bundle; security-sensitive but bounded API
- [ ] **JWK** - JSON Web Key; roll our own (it's a JSON object with specified fields)

---

## Distribution

Implemented:
- [x] **Man pages** - via `paraphase manpage` command

Deferred (needs ecosystem consensus):
- [ ] **Packaging** - cargo-dist, Homebrew formula, AUR package
- [ ] **Release binaries** - pre-built for Linux/macOS/Windows
