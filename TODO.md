# Cambium TODO

## Document Conversion (cambium-document)

Thin integration with a document IR library (separate project).

See `docs/document-ir-spec.md` for comprehensive spec of the document IR:
- Analysis of Pandoc's strengths/weaknesses
- Property-bag based architecture (aligns with Cambium philosophy)
- Layered representation (semantic, style, layout)
- Fidelity tracking for lossy conversions
- Embedded resource handling

**The document IR is out of Cambium's scope** - it's a standalone library project.

cambium-document will:
- [ ] Integrate with document IR library (once it exists)
- [ ] Register format converters with Cambium registry
- [ ] Route document conversions through Cambium's executor

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

## CLI Usability

Implemented:
- [x] **Shell completions** - `cambium completions bash/zsh/fish`
- [x] **Man pages** - `cambium manpage > cambium.1`
- [x] **Verbose/quiet modes** - `-v` for debug info, `-q` for silent
- [x] **Better format detection** - magic bytes before extension fallback
- [x] **Stdin/stdout piping** - `cat file.mp3 | cambium convert - -o - --from mp3 --to wav`
- [x] **Batch processing** - `cambium convert *.mp3 --output-dir out/ --to wav`
- [x] **Progress reporting** - progress bars for batch conversions

Future work:
- [ ] **Presets** - `--preset web` for common conversion profiles
- [ ] **Config file** - `~/.config/cambium/config.toml` for defaults
- [ ] **Better error messages** - actionable suggestions, format hints

## Testing & Quality

Implemented:
- [x] **Integration tests** - 9 end-to-end CLI tests
- [x] **CI/CD** - GitHub Actions for check/test/fmt/clippy/doc/build

Future work:
- [ ] **Benchmarks** - criterion benchmarks for regression tracking

## Distribution

Implemented:
- [x] **Man pages** - via `cambium manpage` command

Future work:
- [ ] **Packaging** - cargo-dist, Homebrew formula, AUR package
- [ ] **Release binaries** - pre-built for Linux/macOS/Windows
