# Open Questions

Unresolved design decisions for Cambium.

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

## Core Model

### How do converters specify cost/quality?

When multiple paths exist (e.g., `PNG → JPG` direct vs `PNG → RGB → JPG`), how to choose?

Options:
1. **Shortest path** - fewest hops
2. **Weighted edges** - converters declare cost (speed? quality loss?)
3. **User hint** - `--prefer lossless` or `--prefer fast`

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

Open:
- Unknown formats: fail? Return minimal `{path: "...", size: N}`?
- Streaming inspection for large files?
- Multiple inspectors match same file? First match? Merge?

## Plugin System

*Plugin format decided: C ABI dynamic libraries. See architecture-decisions.md #001.*

### Plugin versioning

How to handle ABI compatibility?
- Strict version matching (plugin must match exact cambium version)?
- Semver ranges?
- API version number in plugin (current approach in ADR)?

### Plugin dependencies

Can plugins depend on other plugins?
- Plugin A provides `foo → bar`, Plugin B provides `bar → baz`
- What if Plugin B is missing? Graceful degradation or error?

## Incremental Builds

### What's the caching granularity?

Options:
1. **File-level** - mtime/hash per file
2. **Content-addressed** - hash outputs, reuse across projects
3. **Fine-grained** - track dependencies within files

### Where does cache live?

- `.cambium/cache/` in project?
- Global `~/.cache/cambium/`?
- Both with hierarchy?

## CLI Design

### Primary interface

```bash
# Option A: subcommands
cambium convert input.md output.html
cambium pipe input.md | step1 | step2 > output.html
cambium watch src/ --to dist/

# Option B: implicit
cambium input.md output.html  # infers "convert"
cambium input.md --to html    # output to stdout or inferred name

# Option C: make-like
cambium build  # reads cambium.toml, builds all targets
```

### How explicit should type annotation be?

```bash
# Fully inferred
cambium convert data output.yaml

# Explicit source type
cambium convert --from json data output.yaml

# Explicit both
cambium convert --from json --to yaml data output
```

## Integration with Resin/Rhizome

*Library-first decided. See architecture-decisions.md #002.*

### Shared types with Resin?

Do Cambium's `Image`, `Mesh`, etc. share definitions with Resin?
Or is Cambium format-agnostic and Resin provides domain IRs?

Options:
1. **Cambium is format-only** - knows `png`, `obj`, not `Image`, `Mesh`
2. **Shared IR crate** - `rhizome-types` used by both
3. **Cambium defines IRs** - Resin depends on cambium's `Image` type

## Multi-Input / Multi-Output (N→M Conversions)

This is a significant design area that needs careful thought.

### Examples

| Pattern | Example |
|---------|---------|
| N→1 | frames → video, SVGs → icon font, files → manifest |
| 1→N | video → frames, archive → files |
| N→M | batch tree conversion |

### How does this fit property bags?

For 1→1, we have:
```
{format: png, width: 1024} → {format: webp, width: 1024}
```

For N→1, options:
```
# Option A: Array of property bags as input
[{format: png, path: "01.png"}, {format: png, path: "02.png"}, ...] → {format: gif}

# Option B: "Collection" type
{type: collection, items: [...], ...} → {format: gif}

# Option C: Directory as type
{type: directory, path: "frames/", ...} → {format: gif}
```

### How do converters declare multi-input?

```rust
// Current: single input
requires: {format: Exact("png")}

// Multi-input options:
requires: Array({format: Exact("png")})  // array of matching items
requires: {type: "collection", item_format: "png"}  // special collection type
```

### How does search/planning work?

For 1→1: state-space search, straightforward.

For N→1: when does the "aggregation" step happen?
- After all 1→1 conversions complete?
- Need to track "batch" context?

For 1→N: produces multiple outputs, each needs properties.
- Does the converter declare output patterns?
- How does downstream routing work?

## Manifest Generation (Specific Case of N→1)

### Requirements

1. Needs metadata from ALL converted files (not file contents)
2. Must run AFTER individual conversions complete
3. Must NOT include other manifests (avoid recursion)
4. Target-specific (Godot, Unity, custom)

### Options Explored

**Option A: PostProcessor trait (special-cased)**
```rust
trait ManifestGenerator {
    fn includes(&self, props: &Properties) -> bool;
    fn generate(&self, files: &[FileInfo]) -> Result<Vec<u8>>;
}
```
- Con: breaks uniformity, special type of operation

**Option B: N→1 converter (uniform)**
```rust
// Manifest generator is just a converter
requires: [{type: file, is_manifest: false}, ...]  // array input
produces: {format: "godot-import-manifest"}
```
- Pro: fits existing model
- Con: how to express "all files from this batch"?

**Option C: Directory as input type**
```rust
requires: {type: directory, path: "..."}
produces: {format: "godot-import-manifest"}
```
- Pro: uniform, directory is just another type with contents
- Con: contents property could be huge

**Option D: Pipeline phases / hooks**
- Pipeline has stages: inspect → convert → aggregate → finalize
- N→1 converters automatically run in aggregate phase
- Pro: clear ordering without explicit orchestration
- Con: implicit staging rules
- Con: overoptimizing for one case - may not generalize

### The "all files from this batch" problem

How does a manifest generator know which files to include?

```bash
cambium import assets/ --target godot
```

The manifest generator needs to receive:
- All files that were just converted
- Their output paths and properties
- But NOT other manifests or unrelated files

Options:
1. **Implicit batching** - CLI tracks batch, passes to aggregators
2. **Explicit glob** - `requires: {glob: "**/*.{webp,glb,ogg}"}`
3. **Tag/label** - conversions tag outputs, aggregator filters by tag
4. **Scope** - `{scope: "current-batch"}` magic property

### Leaning toward

Probably (B) with (1): N→1 converters are uniform, but CLI provides batching context.

The converter declares it needs an array of file metadata. The CLI/runtime is responsible for collecting the batch and passing it.

```rust
struct ConverterDecl {
    input_cardinality: Cardinality,  // One, Many, OneOrMore
    output_cardinality: Cardinality,
    requires: PropertyPattern,
    produces: PropertyPattern,
}
```

**Needs more design work** - this affects core model significantly.

### Design principles emerging

1. **No special-casing** - sidecars, manifests, etc. are just N→M conversions
2. **Cardinality is declared** - converters say 1→1, 1→N, N→1, N→M
3. **Orchestration collects inputs** - CLI/runtime groups files, passes to converters
4. **User-defined manifests** - manifest format is just another converter output

```bash
# User wants: import files + generate custom JSON manifest
cambium import assets/ --manifest manifest.json

# This runs:
# 1. Individual conversions (1→1 or 1→N each)
# 2. Manifest converter (N→1): [{path, format, ...}, ...] → JSON array
```

### Multi-output and "canonical"

For 1→N, maybe one output is "canonical" (main file) for downstream routing:

```rust
produces: [
    {format: "webp", canonical: true},   // main output
    {format: "json", metadata: true},     // auxiliary
]
```

Or: no distinction, all outputs are equal. User specifies which to use downstream.

**TODO:** Decide if `canonical` flag is needed or if flat list suffices.

### Research needed

- Walk through complete import flow for 2-3 targets (Godot, Unity, custom)
- Identify patterns: what's 1→1, 1→N, N→1, N→M
- Verify framework handles all cases without special-casing
