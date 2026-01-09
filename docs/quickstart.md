# Quickstart

Get up and running with Cambium in minutes.

## Installation

```bash
cargo install cambium-cli
```

This installs Cambium with default converters (common serde and image formats).

### Minimal or Custom Builds

```bash
# Only JSON and PNG
cargo install cambium-cli --no-default-features \
  --features cambium-serde/json,cambium-image/png

# All serde formats, no image support
cargo install cambium-cli --no-default-features --features serde-all

# Everything
cargo install cambium-cli --features all
```

## Basic Usage

### Convert Files

Cambium auto-detects formats from file extensions:

```bash
# Config formats
cambium convert config.json config.yaml
cambium convert settings.yaml settings.toml

# Image formats
cambium convert photo.png photo.webp
cambium convert image.jpg image.gif
```

Override detection with explicit formats:

```bash
cambium convert data.bin output.json --from msgpack --to json
```

### Plan Conversions

See what Cambium will do without executing:

```bash
cambium plan input.json output.toml
```

Output:
```
Planning: json -> toml

Steps:
  1. serde.json-to-toml (default -> default)

Total cost: 1
```

### List Available Converters

```bash
cambium list
```

Shows all registered converters with their input/output properties.

## Workflows

Workflows define pipelines in YAML, TOML, or JSON.

### Simple Workflow

```yaml
# workflow.yaml
source:
  path: input.json
sink:
  path: output.yaml
```

Run with auto-planning:

```bash
cambium run workflow.yaml
```

Cambium finds the conversion path automatically.

### Explicit Steps

For precise control, specify converters:

```yaml
source:
  path: input.json
steps:
  - converter: serde.json-to-yaml
sink:
  path: output.yaml
```

## Library Usage

```rust
use cambium::{Registry, Planner, Properties, PropertyPattern, Cardinality, PropertiesExt};

fn main() -> anyhow::Result<()> {
    // Create registry and register converters
    let mut registry = Registry::new();
    cambium_serde::register_all(&mut registry);
    cambium_image::register_all(&mut registry);

    // Plan a conversion
    let planner = Planner::new(&registry);
    let source = Properties::new().with("format", "json");
    let target = PropertyPattern::new().eq("format", "yaml");

    if let Some(plan) = planner.plan(&source, &target, Cardinality::One, Cardinality::One) {
        println!("Found path with {} steps:", plan.steps.len());
        for step in &plan.steps {
            println!("  {}", step.converter_id);
        }
    }

    Ok(())
}
```

## Next Steps

- [Formats Reference](./formats) - All supported formats
- [Workflow API](./workflow-api) - Full workflow specification
- [Philosophy](./philosophy) - Design principles
- [Use Cases](./use-cases) - Example scenarios
