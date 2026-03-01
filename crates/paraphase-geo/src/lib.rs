//! Geospatial format converters for Paraphase.
//!
//! Provides GPX ↔ GeoJSON conversion.
//!
//! # Features
//! - `gpx` (default) — GPX ↔ GeoJSON via the gpx crate

use paraphase_core::{
    ConvertError, ConvertOutput, Converter, ConverterDecl, Properties, PropertyPattern, Registry,
};

/// Register all enabled geo converters with the registry.
pub fn register_all(registry: &mut Registry) {
    #[cfg(feature = "gpx")]
    {
        registry.register(GpxToGeoJson);
        registry.register(GeoJsonToGpx);
    }
}

// ============================================
// GPX ↔ GeoJSON
// ============================================

#[cfg(feature = "gpx")]
mod gpx_impl {
    use super::*;
    use gpx::{Gpx, GpxVersion, Track, TrackSegment, Waypoint};
    use serde_json::{Value, json};
    use std::io::Cursor;

    /// Convert GPX to GeoJSON FeatureCollection.
    ///
    /// Waypoints → Point features, tracks → LineString features, routes → LineString features.
    pub struct GpxToGeoJson;

    impl Converter for GpxToGeoJson {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "geo.gpx-to-geojson",
                    PropertyPattern::new().eq("format", "gpx"),
                    PropertyPattern::new().eq("format", "geojson"),
                )
                .description("Convert GPX tracks/waypoints to GeoJSON FeatureCollection")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let cursor = Cursor::new(input);
            let gpx_data: Gpx = gpx::read(cursor)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid GPX: {}", e)))?;

            let mut features: Vec<Value> = Vec::new();

            // Convert waypoints to Point features
            for waypoint in &gpx_data.waypoints {
                let point = waypoint.point();
                let mut properties = serde_json::Map::new();
                if let Some(name) = &waypoint.name {
                    properties.insert("name".into(), Value::String(name.clone()));
                }
                if let Some(desc) = &waypoint.description {
                    properties.insert("description".into(), Value::String(desc.clone()));
                }
                if let Some(ele) = waypoint.elevation {
                    properties.insert("elevation".into(), json!(ele));
                }

                let coords = if let Some(ele) = waypoint.elevation {
                    json!([point.x(), point.y(), ele])
                } else {
                    json!([point.x(), point.y()])
                };

                features.push(json!({
                    "type": "Feature",
                    "geometry": {
                        "type": "Point",
                        "coordinates": coords
                    },
                    "properties": properties
                }));
            }

            // Convert tracks to LineString features
            for track in &gpx_data.tracks {
                let mut properties = serde_json::Map::new();
                if let Some(name) = &track.name {
                    properties.insert("name".into(), Value::String(name.clone()));
                }
                if let Some(desc) = &track.description {
                    properties.insert("description".into(), Value::String(desc.clone()));
                }

                for segment in &track.segments {
                    let coords: Vec<Value> = segment
                        .points
                        .iter()
                        .map(|pt| {
                            let p = pt.point();
                            if let Some(ele) = pt.elevation {
                                json!([p.x(), p.y(), ele])
                            } else {
                                json!([p.x(), p.y()])
                            }
                        })
                        .collect();

                    features.push(json!({
                        "type": "Feature",
                        "geometry": {
                            "type": "LineString",
                            "coordinates": coords
                        },
                        "properties": properties
                    }));
                }
            }

            // Convert routes to LineString features
            for route in &gpx_data.routes {
                let mut properties = serde_json::Map::new();
                if let Some(name) = &route.name {
                    properties.insert("name".into(), Value::String(name.clone()));
                }
                if let Some(desc) = &route.description {
                    properties.insert("description".into(), Value::String(desc.clone()));
                }

                let coords: Vec<Value> = route
                    .points
                    .iter()
                    .map(|pt| {
                        let p = pt.point();
                        if let Some(ele) = pt.elevation {
                            json!([p.x(), p.y(), ele])
                        } else {
                            json!([p.x(), p.y()])
                        }
                    })
                    .collect();

                features.push(json!({
                    "type": "Feature",
                    "geometry": {
                        "type": "LineString",
                        "coordinates": coords
                    },
                    "properties": properties
                }));
            }

            let geojson = json!({
                "type": "FeatureCollection",
                "features": features
            });

            let output = serde_json::to_vec_pretty(&geojson)
                .map_err(|e| ConvertError::Failed(format!("JSON serialization failed: {}", e)))?;

            let mut out_props = props.clone();
            out_props.insert("format".into(), "geojson".into());
            Ok(ConvertOutput::Single(output, out_props))
        }
    }

    /// Convert GeoJSON FeatureCollection to GPX.
    ///
    /// Point features → waypoints, LineString features → tracks.
    pub struct GeoJsonToGpx;

    impl Converter for GeoJsonToGpx {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                use paraphase_core::{Predicate, Value as PValue};
                ConverterDecl::simple(
                    "geo.geojson-to-gpx",
                    PropertyPattern::new().with(
                        "format",
                        Predicate::OneOf(vec![PValue::from("geojson"), PValue::from("json")]),
                    ),
                    PropertyPattern::new().eq("format", "gpx"),
                )
                .description("Convert GeoJSON FeatureCollection to GPX")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let geojson: Value = serde_json::from_slice(input)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid JSON: {}", e)))?;

            let features = geojson
                .get("features")
                .and_then(|f| f.as_array())
                .ok_or_else(|| {
                    ConvertError::InvalidInput(
                        "Expected GeoJSON FeatureCollection with features array".into(),
                    )
                })?;

            let mut gpx_data = Gpx {
                version: GpxVersion::Gpx11,
                ..Default::default()
            };

            for feature in features {
                let geometry = match feature.get("geometry") {
                    Some(g) => g,
                    None => continue,
                };
                let geom_type = match geometry.get("type").and_then(|t| t.as_str()) {
                    Some(t) => t,
                    None => continue,
                };
                let feat_props = feature.get("properties");

                match geom_type {
                    "Point" => {
                        let coords = match geometry.get("coordinates").and_then(|c| c.as_array()) {
                            Some(c) => c,
                            None => continue,
                        };
                        if coords.len() < 2 {
                            continue;
                        }
                        let lon = coords[0].as_f64().unwrap_or(0.0);
                        let lat = coords[1].as_f64().unwrap_or(0.0);
                        let ele = coords.get(2).and_then(|e| e.as_f64());

                        let mut wp = Waypoint::new(geo_types::Point::new(lon, lat));
                        wp.elevation = ele;
                        if let Some(name) = feat_props
                            .and_then(|p| p.get("name"))
                            .and_then(|n| n.as_str())
                        {
                            wp.name = Some(name.to_string());
                        }
                        gpx_data.waypoints.push(wp);
                    }
                    "LineString" => {
                        let coords = match geometry.get("coordinates").and_then(|c| c.as_array()) {
                            Some(c) => c,
                            None => continue,
                        };
                        let points: Vec<Waypoint> = coords
                            .iter()
                            .filter_map(|coord| {
                                let arr = coord.as_array()?;
                                if arr.len() < 2 {
                                    return None;
                                }
                                let lon = arr[0].as_f64()?;
                                let lat = arr[1].as_f64()?;
                                let ele = arr.get(2).and_then(|e| e.as_f64());
                                let mut wp = Waypoint::new(geo_types::Point::new(lon, lat));
                                wp.elevation = ele;
                                Some(wp)
                            })
                            .collect();

                        let segment = TrackSegment { points };
                        let mut track = Track::new();
                        if let Some(name) = feat_props
                            .and_then(|p| p.get("name"))
                            .and_then(|n| n.as_str())
                        {
                            track.name = Some(name.to_string());
                        }
                        track.segments.push(segment);
                        gpx_data.tracks.push(track);
                    }
                    _ => {
                        // Skip unsupported geometry types
                    }
                }
            }

            let mut output = Vec::new();
            gpx::write(&gpx_data, &mut output)
                .map_err(|e| ConvertError::Failed(format!("GPX write failed: {}", e)))?;

            let mut out_props = props.clone();
            out_props.insert("format".into(), "gpx".into());
            Ok(ConvertOutput::Single(output, out_props))
        }
    }
}

#[cfg(feature = "gpx")]
pub use gpx_impl::{GeoJsonToGpx, GpxToGeoJson};
