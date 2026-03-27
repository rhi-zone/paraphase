# Open Questions

Unresolved design decisions for Paraphase.

## Resolved

*These are documented elsewhere but listed here for reference.*

- **Type system**: Property bags (ADR-0003)
- **Plugin format**: C ABI dynamic libraries (ADR-0001)
- **Library vs CLI**: Library-first (ADR-0002)
- **Plan vs Suggest**: Just `plan` - incomplete input = suggestion
- **Pattern extraction**: Plugin using regex, not custom DSL
- **Sidecars/manifests**: Just N→M conversions, no special case
- **Workflow format**: Format-agnostic (YAML, TOML, JSON, etc.)
- **Property naming**: Flat by default, namespace when semantics differ
- **Plugin versioning**: Semver ranges (plugin declares compatible paraphase versions)
- **Cache location**: Local by default (`.paraphase/cache/`), global fallback (`~/.cache/paraphase/`), configurable
- **Cache granularity**: Content-addressed with file-level dependency tracking
- **Caching implementation**: Plugin crate (`paraphase-cache`), not baked into core
- **Batch boundaries**: Soft-explicit based on invocation (CLI args = batch, tree = batch, recursive = batch per dir)
- **Converter model**: Named ports with per-port cardinality (`list: bool`), inspired by ComfyUI
- **Planning cardinality**: Inferred from source/target, tracked through graph
- **Expression syntax**: Deferred; use `--optimize quality|speed|size` for MVP, add Dew later if needed

## Streaming / Granular Conversion

**Status:** Not started — needs design before implementation.

### Problem

The current `Converter` trait is fully buffered:

```rust
fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput>;
```

This requires materializing the entire input and output in memory. For large files
(multi-GB NDJSON, audio streams, video frames, huge CSVs) this is a hard ceiling.

### What "streaming" means here

Two distinct capabilities, both needed:

1. **Record-level streaming** — process one logical record at a time (NDJSON line,
   CSV row, subtitle cue, audio frame). Input and output never fully materialize.
   The converter sees `Read`/`Write` or an iterator of chunks.

2. **Pass-through streaming** — for pipelines like `gzip | ndjson→csv | gzip`,
   the executor connects stages with bounded buffers (like Unix pipes) rather than
   temp files or full materialization.

### Which formats can stream?

| Streamable | Not streamable (need full input) |
|------------|----------------------------------|
| NDJSON, CSV, TSV | JSON (tree), YAML (anchors/aliases) |
| SRT, VTT, SBV (cue-at-a-time) | TOML (table merging) |
| Audio frames (WAV, PCM) | Parquet (footer metadata) |
| Line-based text (Base64, Hex) | XLSX/ODS (zip archive) |
| Tar (sequential archive) | Avro (schema in header, blocks are self-contained — partially streamable) |
| Compression (gzip, zstd, brotli) | PNG/JPEG (need full decode for pixel access) |
| PEM (record-at-a-time) | |

Note: serde's `Deserializer`/`Serializer` traits support streaming from `Read`/to
`Write` for formats like JSON and CSV. But our trait boundary forces full buffering
regardless. Even serde-json can stream *tokens* from a `Read` — we just don't let it.

### Open design questions

1. **Trait shape** — options:
   - Separate `StreamingConverter` trait with `Read`/`Write`
   - Add an optional `convert_streaming()` method to existing trait
   - Generic over input type: `convert<R: Read>(input: R, ...)` (breaks object safety)
   - Converter returns a "capability" enum and executor dispatches accordingly

2. **Declaring streamability** — converters need to advertise whether they can stream,
   so the executor/planner can choose buffered vs streaming paths. Probably a flag on
   `ConverterDecl` or `PortDecl`.

3. **Backpressure** — if converter A produces faster than converter B consumes, who
   blocks? Bounded channel? Async streams? This interacts with `BoundedExecutor`'s
   memory budget.

4. **Error semantics** — buffered conversion is atomic (succeed or fail). Streaming
   can fail mid-output. Do we write partial output? Buffer then flush? This is
   format-dependent (appending to CSV is fine; half a zip is not).

5. **Pipeline composition** — a chain like `decompress | parse | transform | serialize | compress`
   should ideally be one connected pipeline, not N intermediate buffers. How does the
   planner fuse adjacent streaming converters?

6. **Plugin ABI** — the C plugin API already has `CAMBIUM_FLAG_STREAMING` but no
   corresponding function signature. Need to define the streaming entry point for plugins.

### Why this matters

Without streaming, paraphase has an effective file-size ceiling equal to available RAM.
This is fine for config files and images, but blocks serious use for:
- Log processing (NDJSON/CSV files can be many GB)
- Audio/video transcoding (even short videos are large uncompressed)
- Archive repacking (tar.gz → tar.zst without extracting to disk)
- Batch pipelines where intermediate results shouldn't materialize

### Approach

Likely path: add an optional `convert_streaming` method that takes `Box<dyn Read>` /
`Box<dyn Write>` and a `supports_streaming() -> bool` query. Executor checks the flag
and dispatches. Non-streaming converters work unchanged — the executor buffers for them.
This is additive; no existing converter breaks.

The `StreamingExecutor` in TODO.md depends on this trait work being done first.

---

## Core Model

### How do converters specify cost/quality?

When multiple paths exist (e.g., `PNG → JPG` direct vs `PNG → RGB → JPG`), how to choose?

**Direction:** Converters declare costs as properties. Users provide scoring expressions.

```rust
struct ConverterDecl {
    // ...existing fields...
    costs: Properties,  // {quality_loss: 0.1, speed: 0.5, ...}
}
```

```bash
paraphase convert a.png b.webp --optimize quality              # minimize quality_loss
paraphase convert a.png b.webp --optimize speed                # minimize speed cost
paraphase convert a.png b.webp --cost "0.7*quality_loss + 0.3*speed"  # weighted
```

**Open:** Expression syntax. Should be consistent across the rhizome ecosystem.

**Ecosystem decision:** Use [Dew](https://github.com/rhi-zone/dew) - minimal expression language for procedural generation.

```
dew-core       # Syntax only: AST, parsing
    |
    +-- dew-scalar     # Scalar domain: f32/f64 math functions
    |                  # Backends: wgsl, lua, cranelift (via features)
    |
    +-- dew-linalg     # Linalg domain: Vec2, Vec3, Mat2, Mat3, etc.
    |
    +-- dew-complex    # Complex numbers
    |
    +-- dew-quaternion # Quaternions
```

Paraphase likely just needs `dew-core` + `dew-scalar` for cost expressions. Each domain crate has self-contained backends (wgsl, lua, cranelift) as features.

### Property naming: what needs namespacing?

**Decision:** Flat by default, namespace only when semantics differ.

Universal (no namespace):
- `width`, `height`, `format`, `path`, `size`
- `quality` (0-100 scale, same meaning everywhere?)

Possibly namespaced:
- `compression` - image lossy compression ≠ archive compression?
- `channels` - audio channels ≠ image channels?

**TODO:** Enumerate and decide.

### Content inspection

How do we populate initial properties from a file?

- Plugins provide inspection: PNG plugin knows how to read PNG metadata
- Returns `Properties` from file bytes

**Concern:** Content inspection for unknown formats ("agent doesn't know, so guess") risks pulling in tons of inspection libraries even as plugins. Need to be intentional about which inspectors are bundled vs opt-in.

Open:
- Unknown formats: fail? Return minimal `{path: "...", size: N}`?
- Streaming inspection for large files?
- Multiple inspectors match same file? First match? Merge?
- Which inspectors are "core" vs plugin-only?

## Plugin System

*Plugin format decided: C ABI dynamic libraries. See architecture-decisions.md #001.*

### Plugin versioning

**Decision:** Semver ranges.

Plugins declare compatible paraphase API versions (e.g., `^1.0`). Paraphase checks compatibility at load time. Breaking API changes bump major version.

```c
// Plugin exports
uint32_t paraphase_plugin_api_version(void);  // e.g., returns 0x010000 for 1.0.0
const char* paraphase_plugin_api_compat(void); // e.g., returns "^1.0"
```

Open:
- Exact compatibility checking semantics
- How to handle plugins built against older minor versions

### Plugin dependencies

Can plugins depend on other plugins?
- Plugin A provides `foo → bar`, Plugin B provides `bar → baz`
- What if Plugin B is missing? Graceful degradation or error?

## Incremental Builds

### Caching strategy

**Decisions:**
- **Granularity:** Content-addressed with file-level dependency tracking
- **Location:** Local by default (`.paraphase/cache/`), global fallback (`~/.cache/paraphase/`), configurable
- **Implementation:** Plugin crate (`paraphase-cache`), not baked into core

**How they compose:**
1. File-level tracking detects "has input changed?" (fast mtime/hash check)
2. Content-addressed lookup finds "have we seen this exact content before?"
3. If CA hit, reuse cached output regardless of project

Fine-grained (sub-file dependencies) adds complexity without proportional benefit. Start with file-level + CA, add fine-grained later if needed.

Core provides hooks for caching; the cache plugin implements the actual storage/lookup.

Open:
- Cache eviction policy (LRU? TTL? size limit?)
- Cache key format (include converter version? options hash?)
- Cross-machine cache sharing (remote cache server?)

## CLI Design

### Primary interface

```bash
# Option A: subcommands
paraphase convert input.md output.html
paraphase pipe input.md | step1 | step2 > output.html
paraphase watch src/ --to dist/

# Option B: implicit
paraphase input.md output.html  # infers "convert"
paraphase input.md --to html    # output to stdout or inferred name

# Option C: make-like
paraphase build  # reads paraphase.toml, builds all targets
```

### How explicit should type annotation be?

```bash
# Fully inferred
paraphase convert data output.yaml

# Explicit source type
paraphase convert --from json data output.yaml

# Explicit both
paraphase convert --from json --to yaml data output
```

## Integration with Resin/Rhizome

*Library-first decided. See architecture-decisions.md #002.*

### Shared types with Resin?

Do Paraphase's `Image`, `Mesh`, etc. share definitions with Resin?
Or is Paraphase format-agnostic and Resin provides domain IRs?

Options:
1. **Paraphase is format-only** - knows `png`, `obj`, not `Image`, `Mesh`
2. **Shared IR crate** - `rhizome-types` used by both
3. **Paraphase defines IRs** - Resin depends on paraphase's `Image` type

## Converter Model (N→M Conversions)

**Prior art:** [ComfyUI](https://github.com/comfyanonymous/ComfyUI) - node-based workflow with named ports, multiple outputs, list handling.

### Named Ports

Converters have named input and output ports, each with a property pattern and cardinality:

```rust
struct ConverterDecl {
    inputs: HashMap<String, PortDecl>,
    outputs: HashMap<String, PortDecl>,
    costs: Properties,  // for path optimization
}

struct PortDecl {
    pattern: PropertyPattern,
    list: bool,  // true = expects/produces list
}
```

### Examples

```rust
// 1→1 (most common)
inputs: { "in": { pattern: {format: "png"}, list: false } }
outputs: { "out": { pattern: {format: "webp"}, list: false } }

// N→1 aggregator (frames → video)
inputs: { "frames": { pattern: {format: "png"}, list: true } }
outputs: { "video": { pattern: {format: "mp4"}, list: false } }

// 1→N expander (video → frames)
inputs: { "video": { pattern: {format: "mp4"}, list: false } }
outputs: { "frames": { pattern: {format: "png"}, list: true } }

// Multiple outputs (image + sidecar)
inputs: { "in": { pattern: {format: "png"}, list: false } }
outputs: {
    "image": { pattern: {format: "webp"}, list: false },
    "sidecar": { pattern: {format: "json"}, list: false }
}

// Multiple inputs (compositing)
inputs: {
    "base": { pattern: {format: "png"}, list: false },
    "overlay": { pattern: {format: "png"}, list: false }
}
outputs: { "out": { pattern: {format: "png"}, list: false } }
```

### Workflow Wiring

Workflows reference specific ports:

```yaml
steps:
  - id: convert
    converter: with-sidecar

  - id: optimize
    converter: webp-optimize
    input: convert.image    # specific output port

  - id: validate
    converter: json-schema
    input: convert.sidecar  # other output port
```

### Cardinality-Aware Planning

Planning infers cardinality from source/target and tracks through the graph:

```
Request: bob_*.png → bob.gif

Inferred:
  - Source: N items (glob)
  - Target: 1 item (single path)

Plan:
  N × {format: png}
         │
         ▼ [resize] (list:false, auto-maps over batch)
  N × {format: png, width: 100}
         │
         ▼ [frames-to-gif] (list:true input, aggregates)
  1 × {format: gif}
         │
         Done!
```

Cardinality transformation rules:
- `list:false → list:false`: maps over N, preserves cardinality
- `list:true → list:false`: consumes N, produces 1 (aggregation)
- `list:false → list:true`: consumes 1, produces N (expansion)
- `list:true → list:true`: consumes N, produces M (transform)

### Design Principles

1. **Named ports** - explicit wiring, no ambiguity for multi-output
2. **Cardinality per-port** - `list: bool`, orthogonal to property pattern
3. **Planning infers cardinality** - from source (glob vs file) and target (single vs pattern)
4. **No special cases** - sidecars, manifests, spritesheets all use same model
5. **Batch context from invocation** - CLI args = batch, tree = batch, recursive = batch per dir
