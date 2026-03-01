//! Subtitle format converters for Paraphase — pure Rust.
//!
//! Implements SRT, WebVTT, and SBV parsing and serialization from scratch.
//!
//! # Features
//! - `srt` (default) — SubRip Text format
//! - `vtt` (default) — Web Video Text Tracks
//! - `sbv` (default) — SubViewer format (YouTube)

use paraphase_core::{
    ConvertError, ConvertOutput, Converter, ConverterDecl, Properties, PropertyPattern, Registry,
};
use std::time::Duration;

/// Register all enabled subtitle converters with the registry.
pub fn register_all(registry: &mut Registry) {
    #[cfg(all(feature = "srt", feature = "vtt"))]
    {
        registry.register(SrtToVtt);
        registry.register(VttToSrt);
    }
    #[cfg(all(feature = "srt", feature = "sbv"))]
    {
        registry.register(SrtToSbv);
        registry.register(SbvToSrt);
    }
    #[cfg(all(feature = "vtt", feature = "sbv"))]
    {
        registry.register(VttToSbv);
        registry.register(SbvToVtt);
    }
}

// ============================================
// Shared data model
// ============================================

/// A single subtitle entry.
#[derive(Debug, Clone)]
pub struct Subtitle {
    /// Sequence number (SRT/SBV: optional, VTT: not present)
    pub index: Option<u64>,
    /// Start time
    pub start: Duration,
    /// End time
    pub end: Duration,
    /// Subtitle text (may contain HTML-like styling tags)
    pub text: String,
}

/// A collection of subtitles.
#[derive(Debug, Clone, Default)]
pub struct SubtitleFile {
    pub entries: Vec<Subtitle>,
}

// ============================================
// Timestamp parsing and formatting
// ============================================

/// Parse SRT timestamp: HH:MM:SS,mmm
fn parse_srt_timestamp(s: &str) -> Option<Duration> {
    let s = s.trim();
    // Format: HH:MM:SS,mmm
    let (time_part, ms_part) = s.split_once(',')?;
    let ms: u64 = ms_part.trim().parse().ok()?;
    let parts: Vec<&str> = time_part.trim().split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let h: u64 = parts[0].parse().ok()?;
    let m: u64 = parts[1].parse().ok()?;
    let sec: u64 = parts[2].parse().ok()?;
    Some(Duration::from_millis(
        h * 3_600_000 + m * 60_000 + sec * 1_000 + ms,
    ))
}

/// Format a Duration as SRT timestamp: HH:MM:SS,mmm
fn format_srt_timestamp(d: Duration) -> String {
    let total_ms = d.as_millis() as u64;
    let ms = total_ms % 1000;
    let total_sec = total_ms / 1000;
    let sec = total_sec % 60;
    let total_min = total_sec / 60;
    let min = total_min % 60;
    let hours = total_min / 60;
    format!("{:02}:{:02}:{:02},{:03}", hours, min, sec, ms)
}

/// Parse VTT timestamp: HH:MM:SS.mmm or MM:SS.mmm
fn parse_vtt_timestamp(s: &str) -> Option<Duration> {
    let s = s.trim();
    let (time_part, ms_part) = s.split_once('.')?;
    let ms: u64 = ms_part.trim().parse().ok()?;
    let parts: Vec<&str> = time_part.trim().split(':').collect();
    let (h, m, sec) = match parts.len() {
        2 => (
            0u64,
            parts[0].parse::<u64>().ok()?,
            parts[1].parse::<u64>().ok()?,
        ),
        3 => (
            parts[0].parse::<u64>().ok()?,
            parts[1].parse::<u64>().ok()?,
            parts[2].parse::<u64>().ok()?,
        ),
        _ => return None,
    };
    Some(Duration::from_millis(
        h * 3_600_000 + m * 60_000 + sec * 1_000 + ms,
    ))
}

/// Format a Duration as VTT timestamp: HH:MM:SS.mmm
fn format_vtt_timestamp(d: Duration) -> String {
    let total_ms = d.as_millis() as u64;
    let ms = total_ms % 1000;
    let total_sec = total_ms / 1000;
    let sec = total_sec % 60;
    let total_min = total_sec / 60;
    let min = total_min % 60;
    let hours = total_min / 60;
    format!("{:02}:{:02}:{:02}.{:03}", hours, min, sec, ms)
}

/// Parse SBV timestamp: H:MM:SS.mmm (hours may be 1+ digits)
fn parse_sbv_timestamp(s: &str) -> Option<Duration> {
    let s = s.trim();
    let (time_part, ms_part) = s.split_once('.')?;
    let ms: u64 = ms_part.trim().parse().ok()?;
    let parts: Vec<&str> = time_part.trim().split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let h: u64 = parts[0].parse().ok()?;
    let m: u64 = parts[1].parse().ok()?;
    let sec: u64 = parts[2].parse().ok()?;
    Some(Duration::from_millis(
        h * 3_600_000 + m * 60_000 + sec * 1_000 + ms,
    ))
}

/// Format a Duration as SBV timestamp: H:MM:SS.mmm
fn format_sbv_timestamp(d: Duration) -> String {
    let total_ms = d.as_millis() as u64;
    let ms = total_ms % 1000;
    let total_sec = total_ms / 1000;
    let sec = total_sec % 60;
    let total_min = total_sec / 60;
    let min = total_min % 60;
    let hours = total_min / 60;
    format!("{}:{:02}:{:02}.{:03}", hours, min, sec, ms)
}

// ============================================
// SRT parser and serializer
// ============================================

/// Parse SRT subtitle format.
pub fn parse_srt(input: &[u8]) -> Result<SubtitleFile, ConvertError> {
    let text = std::str::from_utf8(input)
        .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?;

    // Strip BOM if present
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);

    let mut entries = Vec::new();

    // Split into blocks by one or more blank lines
    for block in text.split("\n\n") {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }

        let mut lines = block.lines();

        // First line: sequence number (optional, might be missing)
        let first = match lines.next() {
            Some(l) => l.trim(),
            None => continue,
        };

        let (index, timing_line) = if let Ok(n) = first.parse::<u64>() {
            // First line is index, next should be timing
            let timing = match lines.next() {
                Some(l) => l.trim(),
                None => continue,
            };
            (Some(n), timing)
        } else if first.contains("-->") {
            // No index, first line is timing
            (None, first)
        } else {
            continue;
        };

        // Parse timing line: start --> end [optional positioning]
        let timing_part = timing_line
            .split_once("-->")
            .map(|(s, e)| (s.trim(), e.trim()));
        let (start_str, end_str) = match timing_part {
            Some((s, e)) => {
                // End might have trailing positioning metadata after space
                let end_clean = e.split_whitespace().next().unwrap_or(e);
                (s, end_clean)
            }
            None => continue,
        };

        let start = match parse_srt_timestamp(start_str) {
            Some(t) => t,
            None => continue,
        };
        let end = match parse_srt_timestamp(end_str) {
            Some(t) => t,
            None => continue,
        };

        let text_lines: Vec<&str> = lines.collect();
        let text = text_lines.join("\n");

        if text.is_empty() && start == end {
            continue;
        }

        entries.push(Subtitle {
            index,
            start,
            end,
            text,
        });
    }

    Ok(SubtitleFile { entries })
}

/// Serialize to SRT format.
pub fn serialize_srt(file: &SubtitleFile) -> String {
    let mut output = String::new();
    for (i, entry) in file.entries.iter().enumerate() {
        let index = entry.index.unwrap_or((i + 1) as u64);
        output.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            index,
            format_srt_timestamp(entry.start),
            format_srt_timestamp(entry.end),
            entry.text
        ));
    }
    output
}

// ============================================
// VTT parser and serializer
// ============================================

/// Parse WebVTT subtitle format.
pub fn parse_vtt(input: &[u8]) -> Result<SubtitleFile, ConvertError> {
    let text = std::str::from_utf8(input)
        .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?;

    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);

    if !text.trim_start().starts_with("WEBVTT") {
        return Err(ConvertError::InvalidInput(
            "VTT file must start with WEBVTT".into(),
        ));
    }

    let mut entries = Vec::new();

    // Skip header block (everything up to first double blank line or first cue)
    let after_header = if let Some(pos) = text.find("\n\n") {
        &text[pos + 2..]
    } else {
        return Ok(SubtitleFile { entries });
    };

    for block in after_header.split("\n\n") {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }

        let mut lines = block.lines().peekable();

        // A block can optionally start with a cue identifier (no "-->")
        let timing_line = {
            let first = match lines.peek() {
                Some(l) => *l,
                None => continue,
            };
            if first.contains("-->") {
                lines.next().unwrap()
            } else {
                // Cue identifier line — skip it
                lines.next();
                match lines.next() {
                    Some(l) => l,
                    None => continue,
                }
            }
        };

        if !timing_line.contains("-->") {
            continue;
        }

        let (start_str, rest) = match timing_line.split_once("-->") {
            Some(p) => p,
            None => continue,
        };
        // End may have trailing positioning settings
        let end_str = rest.split_whitespace().next().unwrap_or(rest.trim());

        let start = match parse_vtt_timestamp(start_str.trim()) {
            Some(t) => t,
            None => continue,
        };
        let end = match parse_vtt_timestamp(end_str) {
            Some(t) => t,
            None => continue,
        };

        let text_lines: Vec<&str> = lines.collect();
        let text = text_lines.join("\n");

        entries.push(Subtitle {
            index: None,
            start,
            end,
            text,
        });
    }

    Ok(SubtitleFile { entries })
}

/// Serialize to WebVTT format.
pub fn serialize_vtt(file: &SubtitleFile) -> String {
    let mut output = String::from("WEBVTT\n\n");
    for entry in &file.entries {
        output.push_str(&format!(
            "{} --> {}\n{}\n\n",
            format_vtt_timestamp(entry.start),
            format_vtt_timestamp(entry.end),
            entry.text
        ));
    }
    output
}

// ============================================
// SBV parser and serializer
// ============================================

/// Parse SBV subtitle format.
pub fn parse_sbv(input: &[u8]) -> Result<SubtitleFile, ConvertError> {
    let text = std::str::from_utf8(input)
        .map_err(|e| ConvertError::InvalidInput(format!("Invalid UTF-8: {}", e)))?;

    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);

    let mut entries = Vec::new();

    for block in text.split("\n\n") {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }

        let mut lines = block.lines();

        // First line: start,end
        let timing_line = match lines.next() {
            Some(l) => l.trim(),
            None => continue,
        };

        let (start_str, end_str) = match timing_line.split_once(',') {
            Some(p) => p,
            None => continue,
        };

        let start = match parse_sbv_timestamp(start_str) {
            Some(t) => t,
            None => continue,
        };
        let end = match parse_sbv_timestamp(end_str) {
            Some(t) => t,
            None => continue,
        };

        let text_lines: Vec<&str> = lines.collect();
        let text = text_lines.join("\n");

        entries.push(Subtitle {
            index: None,
            start,
            end,
            text,
        });
    }

    Ok(SubtitleFile { entries })
}

/// Serialize to SBV format.
pub fn serialize_sbv(file: &SubtitleFile) -> String {
    let mut output = String::new();
    for entry in &file.entries {
        output.push_str(&format!(
            "{},{}\n{}\n\n",
            format_sbv_timestamp(entry.start),
            format_sbv_timestamp(entry.end),
            entry.text
        ));
    }
    output
}

// ============================================
// Converter structs
// ============================================

macro_rules! subtitle_converter {
    ($name:ident, $id:expr, $from:expr, $to:expr, $desc:expr, $parse_fn:ident, $serialize_fn:ident, $out_format:expr) => {
        pub struct $name;

        impl Converter for $name {
            fn decl(&self) -> &ConverterDecl {
                static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
                DECL.get_or_init(|| {
                    ConverterDecl::simple(
                        $id,
                        PropertyPattern::new().eq("format", $from),
                        PropertyPattern::new().eq("format", $to),
                    )
                    .description($desc)
                })
            }

            fn convert(
                &self,
                input: &[u8],
                props: &Properties,
            ) -> Result<ConvertOutput, ConvertError> {
                let parsed = $parse_fn(input)?;
                let serialized = $serialize_fn(&parsed);
                let mut out_props = props.clone();
                out_props.insert("format".into(), $out_format.into());
                Ok(ConvertOutput::Single(serialized.into_bytes(), out_props))
            }
        }
    };
}

#[cfg(all(feature = "srt", feature = "vtt"))]
mod srt_vtt {
    use super::*;

    subtitle_converter!(
        SrtToVtt,
        "subtitle.srt-to-vtt",
        "srt",
        "vtt",
        "Convert SRT subtitles to WebVTT",
        parse_srt,
        serialize_vtt,
        "vtt"
    );

    subtitle_converter!(
        VttToSrt,
        "subtitle.vtt-to-srt",
        "vtt",
        "srt",
        "Convert WebVTT subtitles to SRT",
        parse_vtt,
        serialize_srt,
        "srt"
    );
}

#[cfg(all(feature = "srt", feature = "sbv"))]
mod srt_sbv {
    use super::*;

    subtitle_converter!(
        SrtToSbv,
        "subtitle.srt-to-sbv",
        "srt",
        "sbv",
        "Convert SRT subtitles to SBV",
        parse_srt,
        serialize_sbv,
        "sbv"
    );

    subtitle_converter!(
        SbvToSrt,
        "subtitle.sbv-to-srt",
        "sbv",
        "srt",
        "Convert SBV subtitles to SRT",
        parse_sbv,
        serialize_srt,
        "srt"
    );
}

#[cfg(all(feature = "vtt", feature = "sbv"))]
mod vtt_sbv {
    use super::*;

    subtitle_converter!(
        VttToSbv,
        "subtitle.vtt-to-sbv",
        "vtt",
        "sbv",
        "Convert WebVTT subtitles to SBV",
        parse_vtt,
        serialize_sbv,
        "sbv"
    );

    subtitle_converter!(
        SbvToVtt,
        "subtitle.sbv-to-vtt",
        "sbv",
        "vtt",
        "Convert SBV subtitles to WebVTT",
        parse_sbv,
        serialize_vtt,
        "vtt"
    );
}

#[cfg(all(feature = "srt", feature = "vtt"))]
pub use srt_vtt::{SrtToVtt, VttToSrt};

#[cfg(all(feature = "srt", feature = "sbv"))]
pub use srt_sbv::{SbvToSrt, SrtToSbv};

#[cfg(all(feature = "vtt", feature = "sbv"))]
pub use vtt_sbv::{SbvToVtt, VttToSbv};

#[cfg(test)]
mod tests {
    use super::*;

    const SRT_SAMPLE: &str = "1\n00:00:01,000 --> 00:00:04,000\nHello, world!\n\n2\n00:00:05,000 --> 00:00:08,000\nSecond subtitle.\n\n";
    const VTT_SAMPLE: &str = "WEBVTT\n\n00:00:01.000 --> 00:00:04.000\nHello, world!\n\n00:00:05.000 --> 00:00:08.000\nSecond subtitle.\n\n";
    const SBV_SAMPLE: &str =
        "0:00:01.000,0:00:04.000\nHello, world!\n\n0:00:05.000,0:00:08.000\nSecond subtitle.\n\n";

    #[test]
    fn test_parse_srt() {
        let file = parse_srt(SRT_SAMPLE.as_bytes()).unwrap();
        assert_eq!(file.entries.len(), 2);
        assert_eq!(file.entries[0].index, Some(1));
        assert_eq!(file.entries[0].start, Duration::from_millis(1000));
        assert_eq!(file.entries[0].end, Duration::from_millis(4000));
        assert_eq!(file.entries[0].text, "Hello, world!");
    }

    #[test]
    fn test_parse_vtt() {
        let file = parse_vtt(VTT_SAMPLE.as_bytes()).unwrap();
        assert_eq!(file.entries.len(), 2);
        assert_eq!(file.entries[0].start, Duration::from_millis(1000));
        assert_eq!(file.entries[0].text, "Hello, world!");
    }

    #[test]
    fn test_parse_sbv() {
        let file = parse_sbv(SBV_SAMPLE.as_bytes()).unwrap();
        assert_eq!(file.entries.len(), 2);
        assert_eq!(file.entries[0].start, Duration::from_millis(1000));
        assert_eq!(file.entries[0].text, "Hello, world!");
    }

    #[test]
    fn test_srt_roundtrip() {
        let file = parse_srt(SRT_SAMPLE.as_bytes()).unwrap();
        let out = serialize_srt(&file);
        let file2 = parse_srt(out.as_bytes()).unwrap();
        assert_eq!(file.entries.len(), file2.entries.len());
        for (a, b) in file.entries.iter().zip(file2.entries.iter()) {
            assert_eq!(a.start, b.start);
            assert_eq!(a.end, b.end);
            assert_eq!(a.text, b.text);
        }
    }

    #[test]
    #[cfg(all(feature = "srt", feature = "vtt"))]
    fn test_srt_to_vtt_converter() {
        use paraphase_core::PropertiesExt;
        let props = Properties::new().with("format", "srt");
        let result = SrtToVtt.convert(SRT_SAMPLE.as_bytes(), &props).unwrap();
        let (out, out_props) = match result {
            ConvertOutput::Single(b, p) => (b, p),
            _ => panic!("Expected single"),
        };
        assert_eq!(
            out_props.get("format").and_then(|v| v.as_str()),
            Some("vtt")
        );
        let out_str = std::str::from_utf8(&out).unwrap();
        assert!(out_str.starts_with("WEBVTT"));
        assert!(out_str.contains("00:00:01.000 --> 00:00:04.000"));
        assert!(out_str.contains("Hello, world!"));
    }
}
