//! Plugin authoring helpers for Cambium.
//!
//! This crate provides utilities for writing Cambium plugins,
//! including the C ABI exports and procedural macros.

pub use cambium::{
    ConvertError, ConvertOutput, Converter, ConverterDecl, PortDecl, Predicate, Properties,
    PropertiesExt, PropertyPattern, Value,
};

// TODO: Add #[cambium_converter] proc macro
// TODO: Add C ABI export helpers
