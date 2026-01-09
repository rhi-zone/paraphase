# Cambium TODO

## Document Conversion (cambium-document)

New crate for document → PDF normalization:

- [ ] **Design ADR** - intermediate representation, format support
- [ ] **Pure Rust core** - TXT, images → PDF via `printpdf`/`genpdf`
- [ ] **Markdown → PDF** - via Typst (pure Rust) or Pandoc
- [ ] **HTML → PDF** - Typst, or external (wkhtmltopdf/WeasyPrint)
- [ ] **Office formats** - LibreOffice headless (DOCX, XLSX, ODT, etc.)
- [ ] **PDF as input** - extract text, images via `pdf-extract`/`lopdf`

Typst is promising for pure Rust PDF generation with proper layout.

## Audio Encoders (cambium-audio)

Currently only WAV encoding is supported. Adding encoders for other formats:

- [ ] **FLAC encoder** - pure Rust via `flacenc` crate (if stable)
- [ ] **MP3 encoder** - requires `lame` (native dependency)
- [ ] **OGG Vorbis encoder** - requires `libvorbis` (native dependency)
- [ ] **AAC encoder** - requires FFmpeg or native lib
- [ ] **Opus encoder** - consider as modern alternative to OGG

## Video (cambium-video)

- [ ] Complete frame encoding pipeline (currently scaffold)
- [ ] Audio track passthrough/transcoding
- [ ] Subtitle extraction

## Architecture

See ADR-0006 for the Executor abstraction.

Implemented:
- [x] **SimpleExecutor** - sequential, unbounded memory
- [x] **BoundedExecutor** - sequential with memory limit checking (fail-fast)
- [x] **ParallelExecutor** - rayon + memory budget for batch (requires `parallel` feature)
- [x] **MemoryBudget** - semaphore-like reservation with RAII permits

Future work:
- [ ] **StreamingExecutor** - chunk-based I/O for huge files (requires converter interface changes)

## General

- [ ] Batch processing in CLI (`cambium convert *.mp3 --to wav`)
- [ ] Streaming/pipe support for large files
- [ ] Progress reporting for long conversions
