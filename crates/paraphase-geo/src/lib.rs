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
    #[cfg(feature = "wkt")]
    {
        registry.register(WktToGeoJson);
        registry.register(GeoJsonToWkt);
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

// ============================================
// WKT ↔ GeoJSON
// ============================================

#[cfg(feature = "wkt")]
mod wkt_impl {
    use super::*;
    use serde_json::{Value, json};
    use wkt::TryFromWkt;

    /// Convert WKT geometry to a GeoJSON geometry object.
    pub struct WktToGeoJson;

    impl Converter for WktToGeoJson {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "geo.wkt-to-geojson",
                    PropertyPattern::new().eq("format", "wkt"),
                    PropertyPattern::new().eq("format", "geojson"),
                )
                .description("Convert WKT geometry to GeoJSON geometry object")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let wkt_str = std::str::from_utf8(input)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?;

            let geom: geo_types::Geometry<f64> = geo_types::Geometry::try_from_wkt_str(wkt_str)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid WKT: {}", e)))?;

            let geojson = geo_to_geojson(&geom);
            let output = serde_json::to_vec_pretty(&geojson)
                .map_err(|e| ConvertError::Failed(format!("JSON serialization failed: {}", e)))?;

            let mut out_props = props.clone();
            out_props.insert("format".into(), "geojson".into());
            Ok(ConvertOutput::Single(output, out_props))
        }
    }

    /// Convert a GeoJSON geometry object to WKT.
    pub struct GeoJsonToWkt;

    impl Converter for GeoJsonToWkt {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                use paraphase_core::{Predicate, Value as PValue};
                ConverterDecl::simple(
                    "geo.geojson-to-wkt",
                    PropertyPattern::new().with(
                        "format",
                        Predicate::OneOf(vec![PValue::from("geojson"), PValue::from("json")]),
                    ),
                    PropertyPattern::new().eq("format", "wkt"),
                )
                .description("Convert GeoJSON geometry object to WKT")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let geojson: Value = serde_json::from_slice(input)
                .map_err(|e| ConvertError::InvalidInput(format!("Invalid JSON: {}", e)))?;

            // Accept a bare geometry or a Feature wrapping one
            let geometry = if geojson.get("type").and_then(|t| t.as_str()) == Some("Feature") {
                geojson
                    .get("geometry")
                    .ok_or_else(|| ConvertError::InvalidInput("Feature has no geometry".into()))?
                    .clone()
            } else {
                geojson
            };

            let wkt_str = geojson_to_wkt(&geometry)?;
            let mut out_props = props.clone();
            out_props.insert("format".into(), "wkt".into());
            Ok(ConvertOutput::Single(wkt_str.into_bytes(), out_props))
        }
    }

    fn geo_to_geojson(geom: &geo_types::Geometry<f64>) -> Value {
        match geom {
            geo_types::Geometry::Point(p) => json!({
                "type": "Point",
                "coordinates": [p.x(), p.y()]
            }),
            geo_types::Geometry::Line(l) => json!({
                "type": "LineString",
                "coordinates": [[l.start.x, l.start.y], [l.end.x, l.end.y]]
            }),
            geo_types::Geometry::LineString(ls) => {
                let coords: Vec<Value> = ls.coords().map(|c| json!([c.x, c.y])).collect();
                json!({ "type": "LineString", "coordinates": coords })
            }
            geo_types::Geometry::Polygon(poly) => {
                let rings = polygon_rings(poly);
                json!({ "type": "Polygon", "coordinates": rings })
            }
            geo_types::Geometry::MultiPoint(mp) => {
                let coords: Vec<Value> = mp.0.iter().map(|p| json!([p.x(), p.y()])).collect();
                json!({ "type": "MultiPoint", "coordinates": coords })
            }
            geo_types::Geometry::MultiLineString(mls) => {
                let lines: Vec<Value> = mls
                    .0
                    .iter()
                    .map(|ls| {
                        let coords: Vec<Value> = ls.coords().map(|c| json!([c.x, c.y])).collect();
                        Value::Array(coords)
                    })
                    .collect();
                json!({ "type": "MultiLineString", "coordinates": lines })
            }
            geo_types::Geometry::MultiPolygon(mp) => {
                let polys: Vec<Value> = mp.0.iter().map(polygon_rings).collect();
                json!({ "type": "MultiPolygon", "coordinates": polys })
            }
            geo_types::Geometry::GeometryCollection(gc) => {
                let geometries: Vec<Value> = gc.0.iter().map(geo_to_geojson).collect();
                json!({ "type": "GeometryCollection", "geometries": geometries })
            }
            geo_types::Geometry::Rect(r) => {
                // Represent as a Polygon (5-point ring)
                let min = r.min();
                let max = r.max();
                json!({
                    "type": "Polygon",
                    "coordinates": [[
                        [min.x, min.y],
                        [max.x, min.y],
                        [max.x, max.y],
                        [min.x, max.y],
                        [min.x, min.y]
                    ]]
                })
            }
            geo_types::Geometry::Triangle(t) => {
                json!({
                    "type": "Polygon",
                    "coordinates": [[
                        [t.0.x, t.0.y],
                        [t.1.x, t.1.y],
                        [t.2.x, t.2.y],
                        [t.0.x, t.0.y]
                    ]]
                })
            }
        }
    }

    fn polygon_rings(poly: &geo_types::Polygon<f64>) -> Value {
        let exterior: Vec<Value> = poly
            .exterior()
            .coords()
            .map(|c| json!([c.x, c.y]))
            .collect();
        let mut rings = vec![Value::Array(exterior)];
        for interior in poly.interiors() {
            let ring: Vec<Value> = interior.coords().map(|c| json!([c.x, c.y])).collect();
            rings.push(Value::Array(ring));
        }
        Value::Array(rings)
    }

    fn geojson_to_wkt(geometry: &Value) -> Result<String, ConvertError> {
        let geom_type = geometry
            .get("type")
            .and_then(|t| t.as_str())
            .ok_or_else(|| ConvertError::InvalidInput("Missing geometry 'type'".into()))?;

        match geom_type {
            "Point" => {
                let coords = get_coords(geometry)?;
                let arr = coords.as_array().ok_or_else(|| {
                    ConvertError::InvalidInput("Point coordinates must be array".into())
                })?;
                Ok(format!("POINT({} {})", num(arr, 0), num(arr, 1)))
            }
            "LineString" => {
                let coords = get_coords(geometry)?;
                let pts = coords_to_wkt_pts(coords.as_array().ok_or_else(|| {
                    ConvertError::InvalidInput("LineString coordinates must be array".into())
                })?);
                Ok(format!("LINESTRING({})", pts))
            }
            "Polygon" => {
                let coords = get_coords(geometry)?;
                let rings = coords.as_array().ok_or_else(|| {
                    ConvertError::InvalidInput("Polygon coordinates must be array".into())
                })?;
                let ring_strs: Vec<String> = rings
                    .iter()
                    .map(|r| {
                        let pts = r
                            .as_array()
                            .map(|a| coords_to_wkt_pts(a))
                            .unwrap_or_default();
                        format!("({})", pts)
                    })
                    .collect();
                Ok(format!("POLYGON({})", ring_strs.join(", ")))
            }
            "MultiPoint" => {
                let coords = get_coords(geometry)?;
                let pts: Vec<String> = coords
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .map(|c| {
                        let a = c.as_array().map(|a| a.as_slice()).unwrap_or(&[]);
                        format!("({} {})", num(a, 0), num(a, 1))
                    })
                    .collect();
                Ok(format!("MULTIPOINT({})", pts.join(", ")))
            }
            "MultiLineString" => {
                let coords = get_coords(geometry)?;
                let lines: Vec<String> = coords
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .map(|ls| {
                        let pts = ls
                            .as_array()
                            .map(|a| coords_to_wkt_pts(a))
                            .unwrap_or_default();
                        format!("({})", pts)
                    })
                    .collect();
                Ok(format!("MULTILINESTRING({})", lines.join(", ")))
            }
            "MultiPolygon" => {
                let coords = get_coords(geometry)?;
                let polys: Vec<String> = coords
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .map(|poly| {
                        let rings: Vec<String> = poly
                            .as_array()
                            .unwrap_or(&vec![])
                            .iter()
                            .map(|r| {
                                let pts = r
                                    .as_array()
                                    .map(|a| coords_to_wkt_pts(a))
                                    .unwrap_or_default();
                                format!("({})", pts)
                            })
                            .collect();
                        format!("({})", rings.join(", "))
                    })
                    .collect();
                Ok(format!("MULTIPOLYGON({})", polys.join(", ")))
            }
            "GeometryCollection" => {
                let geometries = geometry
                    .get("geometries")
                    .and_then(|g| g.as_array())
                    .ok_or_else(|| {
                        ConvertError::InvalidInput("GeometryCollection missing 'geometries'".into())
                    })?;
                let parts: Result<Vec<String>, ConvertError> =
                    geometries.iter().map(geojson_to_wkt).collect();
                Ok(format!("GEOMETRYCOLLECTION({})", parts?.join(", ")))
            }
            other => Err(ConvertError::InvalidInput(format!(
                "Unsupported geometry type: {}",
                other
            ))),
        }
    }

    fn get_coords(geometry: &Value) -> Result<&Value, ConvertError> {
        geometry
            .get("coordinates")
            .ok_or_else(|| ConvertError::InvalidInput("Missing 'coordinates' in geometry".into()))
    }

    fn num(arr: &[Value], idx: usize) -> f64 {
        arr.get(idx).and_then(|v| v.as_f64()).unwrap_or(0.0)
    }

    fn coords_to_wkt_pts(coords: &[Value]) -> String {
        coords
            .iter()
            .map(|c| {
                let a = c.as_array().map(|a| a.as_slice()).unwrap_or(&[]);
                format!("{} {}", num(a, 0), num(a, 1))
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[cfg(feature = "wkt")]
pub use wkt_impl::{GeoJsonToWkt, WktToGeoJson};
