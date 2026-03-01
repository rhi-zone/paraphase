//! Font format converters for Paraphase.
//!
//! Implements TTF/OTF ↔ WOFF1 conversion in pure Rust using flate2 for
//! zlib compression. WOFF1 is a straightforward binary container for
//! compressed sfnt (TTF/OTF) data.
//!
//! # Features
//! - `woff` (default) — TTF/OTF ↔ WOFF1

use paraphase_core::{
    ConvertError, ConvertOutput, Converter, ConverterDecl, Properties, PropertyPattern, Registry,
};

/// Register all enabled font converters with the registry.
pub fn register_all(registry: &mut Registry) {
    #[cfg(feature = "woff")]
    {
        registry.register(TtfToWoff);
        registry.register(OtfToWoff);
        registry.register(WoffToTtf);
    }
}

// ============================================
// WOFF1 format implementation
//
// Spec: https://www.w3.org/TR/WOFF/
//
// WOFF header (44 bytes):
//   signature:       uint32 = 0x774F4646 ('wOFF')
//   flavor:          uint32 (copy from sfnt sfVersion)
//   length:          uint32 (total WOFF file size)
//   numTables:       uint16
//   reserved:        uint16 = 0
//   totalSfntSize:   uint32
//   majorVersion:    uint16
//   minorVersion:    uint16
//   metaOffset:      uint32 = 0
//   metaLength:      uint32 = 0
//   metaOrigLength:  uint32 = 0
//   privOffset:      uint32 = 0
//   privLength:      uint32 = 0
//
// WOFF table directory (20 bytes each):
//   tag:          uint32 (4-char tag)
//   offset:       uint32 (position in WOFF file)
//   compLength:   uint32 (compressed size)
//   origLength:   uint32 (uncompressed size)
//   origChecksum: uint32
//
// sfnt offset table (12 bytes):
//   sfVersion:     uint32
//   numTables:     uint16
//   searchRange:   uint16
//   entrySelector: uint16
//   rangeShift:    uint16
//
// sfnt table directory (16 bytes each):
//   tag:      4 bytes
//   checksum: uint32
//   offset:   uint32
//   length:   uint32
// ============================================

#[cfg(feature = "woff")]
mod woff_impl {
    use super::*;
    use flate2::Compression;
    use flate2::read::DeflateDecoder;
    use flate2::write::DeflateEncoder;
    use std::io::{Read, Write};

    const WOFF_SIGNATURE: u32 = 0x774F4646;
    const WOFF_HEADER_SIZE: usize = 44;
    const WOFF_TABLE_ENTRY_SIZE: usize = 20;
    const SFNT_OFFSET_TABLE_SIZE: usize = 12;
    const SFNT_TABLE_ENTRY_SIZE: usize = 16;

    fn read_u16_be(data: &[u8], offset: usize) -> Option<u16> {
        if offset + 2 > data.len() {
            return None;
        }
        Some(u16::from_be_bytes([data[offset], data[offset + 1]]))
    }

    fn read_u32_be(data: &[u8], offset: usize) -> Option<u32> {
        if offset + 4 > data.len() {
            return None;
        }
        Some(u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]))
    }

    fn write_u16_be(buf: &mut Vec<u8>, v: u16) {
        buf.extend_from_slice(&v.to_be_bytes());
    }

    fn write_u32_be(buf: &mut Vec<u8>, v: u32) {
        buf.extend_from_slice(&v.to_be_bytes());
    }

    /// Pad length to a multiple of 4.
    fn pad4(n: usize) -> usize {
        (n + 3) & !3
    }

    #[derive(Debug)]
    struct SfntTable {
        tag: [u8; 4],
        checksum: u32,
        data: Vec<u8>,
    }

    /// Parse the sfnt table directory from a TTF/OTF file.
    fn parse_sfnt_tables(data: &[u8]) -> Result<(u32, Vec<SfntTable>), ConvertError> {
        if data.len() < SFNT_OFFSET_TABLE_SIZE {
            return Err(ConvertError::InvalidInput("Font file too short".into()));
        }

        let sf_version = read_u32_be(data, 0)
            .ok_or_else(|| ConvertError::InvalidInput("Cannot read sfnt version".into()))?;
        let num_tables = read_u16_be(data, 4)
            .ok_or_else(|| ConvertError::InvalidInput("Cannot read numTables".into()))?
            as usize;

        if data.len() < SFNT_OFFSET_TABLE_SIZE + num_tables * SFNT_TABLE_ENTRY_SIZE {
            return Err(ConvertError::InvalidInput(
                "Font file truncated at table directory".into(),
            ));
        }

        let mut tables = Vec::with_capacity(num_tables);
        let dir_base = SFNT_OFFSET_TABLE_SIZE;

        for i in 0..num_tables {
            let entry_base = dir_base + i * SFNT_TABLE_ENTRY_SIZE;
            let tag: [u8; 4] = data[entry_base..entry_base + 4].try_into().unwrap();
            let checksum = read_u32_be(data, entry_base + 4).unwrap_or(0);
            let offset = read_u32_be(data, entry_base + 8).unwrap_or(0) as usize;
            let length = read_u32_be(data, entry_base + 12).unwrap_or(0) as usize;

            if offset + length > data.len() {
                return Err(ConvertError::InvalidInput(format!(
                    "Table '{}' extends beyond file end",
                    String::from_utf8_lossy(&tag)
                )));
            }

            let table_data = data[offset..offset + length].to_vec();
            tables.push(SfntTable {
                tag,
                checksum,
                data: table_data,
            });
        }

        // Sort tables by tag (WOFF requires sorted order)
        tables.sort_by(|a, b| a.tag.cmp(&b.tag));

        Ok((sf_version, tables))
    }

    /// Build a WOFF1 file from sfnt tables.
    fn build_woff(sf_version: u32, tables: &[SfntTable]) -> Result<Vec<u8>, ConvertError> {
        let num_tables = tables.len() as u16;

        // Compress each table
        let mut compressed: Vec<Vec<u8>> = Vec::with_capacity(tables.len());
        for table in tables {
            // Compress with zlib deflate
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
            encoder
                .write_all(&table.data)
                .map_err(|e| ConvertError::Failed(format!("Compression failed: {}", e)))?;
            let comp_data = encoder
                .finish()
                .map_err(|e| ConvertError::Failed(format!("Compression finish failed: {}", e)))?;

            // Only use compressed form if it's smaller
            if comp_data.len() < table.data.len() {
                compressed.push(comp_data);
            } else {
                compressed.push(table.data.clone());
            }
        }

        // Calculate total sfnt size (uncompressed)
        let total_sfnt_size = {
            // sfnt offset table + table directory + table data (padded)
            let mut size = SFNT_OFFSET_TABLE_SIZE + tables.len() * SFNT_TABLE_ENTRY_SIZE;
            for table in tables {
                size += pad4(table.data.len());
            }
            size as u32
        };

        // Calculate total WOFF file size
        let tables_start = WOFF_HEADER_SIZE + tables.len() * WOFF_TABLE_ENTRY_SIZE;
        let mut data_offset = tables_start;
        let mut offsets = Vec::with_capacity(tables.len());
        for comp in &compressed {
            offsets.push(data_offset as u32);
            data_offset += pad4(comp.len());
        }
        let total_length = data_offset as u32;

        let mut buf = Vec::with_capacity(total_length as usize);

        // Write WOFF header
        write_u32_be(&mut buf, WOFF_SIGNATURE);
        write_u32_be(&mut buf, sf_version);
        write_u32_be(&mut buf, total_length);
        write_u16_be(&mut buf, num_tables);
        write_u16_be(&mut buf, 0); // reserved
        write_u32_be(&mut buf, total_sfnt_size);
        write_u16_be(&mut buf, 1); // majorVersion
        write_u16_be(&mut buf, 0); // minorVersion
        write_u32_be(&mut buf, 0); // metaOffset
        write_u32_be(&mut buf, 0); // metaLength
        write_u32_be(&mut buf, 0); // metaOrigLength
        write_u32_be(&mut buf, 0); // privOffset
        write_u32_be(&mut buf, 0); // privLength

        // Write WOFF table directory
        for (i, table) in tables.iter().enumerate() {
            buf.extend_from_slice(&table.tag);
            write_u32_be(&mut buf, offsets[i]);
            write_u32_be(&mut buf, compressed[i].len() as u32);
            write_u32_be(&mut buf, table.data.len() as u32);
            write_u32_be(&mut buf, table.checksum);
        }

        // Write table data (padded to 4-byte boundaries)
        for comp in &compressed {
            buf.extend_from_slice(comp);
            let pad = pad4(comp.len()) - comp.len();
            buf.extend(std::iter::repeat_n(0u8, pad));
        }

        Ok(buf)
    }

    /// Reconstruct a TTF/OTF from a WOFF1 file.
    fn woff_to_sfnt(data: &[u8]) -> Result<Vec<u8>, ConvertError> {
        if data.len() < WOFF_HEADER_SIZE {
            return Err(ConvertError::InvalidInput("WOFF file too short".into()));
        }

        let signature = read_u32_be(data, 0)
            .ok_or_else(|| ConvertError::InvalidInput("Cannot read WOFF signature".into()))?;
        if signature != WOFF_SIGNATURE {
            return Err(ConvertError::InvalidInput(
                "Not a WOFF file (bad magic)".into(),
            ));
        }

        let sf_version = read_u32_be(data, 4).unwrap_or(0x00010000);
        let num_tables = read_u16_be(data, 12).unwrap_or(0) as usize;

        if data.len() < WOFF_HEADER_SIZE + num_tables * WOFF_TABLE_ENTRY_SIZE {
            return Err(ConvertError::InvalidInput(
                "WOFF file truncated at table directory".into(),
            ));
        }

        // Read table directory
        let mut tables: Vec<(u32, u32, u32, u32, u32)> = Vec::with_capacity(num_tables); // (tag_u32, offset, comp_len, orig_len, checksum)
        let dir_base = WOFF_HEADER_SIZE;
        for i in 0..num_tables {
            let entry_base = dir_base + i * WOFF_TABLE_ENTRY_SIZE;
            let tag = read_u32_be(data, entry_base).unwrap_or(0);
            let offset = read_u32_be(data, entry_base + 4).unwrap_or(0);
            let comp_len = read_u32_be(data, entry_base + 8).unwrap_or(0);
            let orig_len = read_u32_be(data, entry_base + 12).unwrap_or(0);
            let checksum = read_u32_be(data, entry_base + 16).unwrap_or(0);
            tables.push((tag, offset, comp_len, orig_len, checksum));
        }

        // Decompress each table
        let mut decompressed: Vec<(u32, u32, Vec<u8>)> = Vec::with_capacity(num_tables); // (tag, checksum, data)
        for (tag, offset, comp_len, orig_len, checksum) in &tables {
            let start = *offset as usize;
            let end = start + *comp_len as usize;
            if end > data.len() {
                return Err(ConvertError::InvalidInput(
                    "WOFF table data out of bounds".into(),
                ));
            }
            let comp_data = &data[start..end];

            let table_data = if *comp_len == *orig_len {
                // Not compressed (stored as-is)
                comp_data.to_vec()
            } else {
                // Decompress with zlib deflate
                let mut decoder = DeflateDecoder::new(comp_data);
                let mut decompressed_data = Vec::new();
                decoder
                    .read_to_end(&mut decompressed_data)
                    .map_err(|e| ConvertError::Failed(format!("Decompression failed: {}", e)))?;
                if decompressed_data.len() != *orig_len as usize {
                    return Err(ConvertError::Failed("Decompressed size mismatch".into()));
                }
                decompressed_data
            };

            decompressed.push((*tag, *checksum, table_data));
        }

        // Sort by tag (restore canonical order)
        decompressed.sort_by_key(|(tag, _, _)| *tag);

        let n = decompressed.len() as u16;

        // Calculate sfnt table offsets
        let dir_size = SFNT_OFFSET_TABLE_SIZE + decompressed.len() * SFNT_TABLE_ENTRY_SIZE;
        let mut sfnt_offsets = Vec::with_capacity(decompressed.len());
        let mut current_offset = dir_size;
        for (_, _, tdata) in &decompressed {
            sfnt_offsets.push(current_offset as u32);
            current_offset += pad4(tdata.len());
        }

        // sfnt header fields
        let search_range = {
            let mut sr = 1u16;
            while sr * 2 <= n {
                sr *= 2;
            }
            sr * 16
        };
        let entry_selector = {
            let mut es = 0u16;
            let mut x = n;
            while x > 1 {
                x >>= 1;
                es += 1;
            }
            es
        };
        let range_shift = n * 16 - search_range;

        let mut buf = Vec::new();

        // Write sfnt offset table
        write_u32_be(&mut buf, sf_version);
        write_u16_be(&mut buf, n);
        write_u16_be(&mut buf, search_range);
        write_u16_be(&mut buf, entry_selector);
        write_u16_be(&mut buf, range_shift);

        // Write sfnt table directory
        for (i, (tag, checksum, tdata)) in decompressed.iter().enumerate() {
            buf.extend_from_slice(&tag.to_be_bytes());
            write_u32_be(&mut buf, *checksum);
            write_u32_be(&mut buf, sfnt_offsets[i]);
            write_u32_be(&mut buf, tdata.len() as u32);
        }

        // Write table data (padded)
        for (_, _, tdata) in &decompressed {
            buf.extend_from_slice(tdata);
            let pad = pad4(tdata.len()) - tdata.len();
            buf.extend(std::iter::repeat_n(0u8, pad));
        }

        Ok(buf)
    }

    /// Convert TTF to WOFF1.
    pub struct TtfToWoff;

    impl Converter for TtfToWoff {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "font.ttf-to-woff",
                    PropertyPattern::new().eq("format", "ttf"),
                    PropertyPattern::new().eq("format", "woff"),
                )
                .description("Convert TTF font to WOFF1 container")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let (sf_version, tables) = parse_sfnt_tables(input)?;
            let woff = build_woff(sf_version, &tables)?;
            let mut out_props = props.clone();
            out_props.insert("format".into(), "woff".into());
            Ok(ConvertOutput::Single(woff, out_props))
        }
    }

    /// Convert OTF to WOFF1.
    pub struct OtfToWoff;

    impl Converter for OtfToWoff {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "font.otf-to-woff",
                    PropertyPattern::new().eq("format", "otf"),
                    PropertyPattern::new().eq("format", "woff"),
                )
                .description("Convert OTF font to WOFF1 container")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let (sf_version, tables) = parse_sfnt_tables(input)?;
            let woff = build_woff(sf_version, &tables)?;
            let mut out_props = props.clone();
            out_props.insert("format".into(), "woff".into());
            Ok(ConvertOutput::Single(woff, out_props))
        }
    }

    /// Convert WOFF1 back to TTF.
    pub struct WoffToTtf;

    impl Converter for WoffToTtf {
        fn decl(&self) -> &ConverterDecl {
            static DECL: std::sync::OnceLock<ConverterDecl> = std::sync::OnceLock::new();
            DECL.get_or_init(|| {
                ConverterDecl::simple(
                    "font.woff-to-ttf",
                    PropertyPattern::new().eq("format", "woff"),
                    PropertyPattern::new().eq("format", "ttf"),
                )
                .description("Extract TTF from WOFF1 container")
            })
        }

        fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
            let ttf = woff_to_sfnt(input)?;
            let mut out_props = props.clone();
            out_props.insert("format".into(), "ttf".into());
            Ok(ConvertOutput::Single(ttf, out_props))
        }
    }
}

#[cfg(feature = "woff")]
pub use woff_impl::{OtfToWoff, TtfToWoff, WoffToTtf};

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(feature = "woff")]
    fn test_woff_bad_magic() {
        use super::*;
        use paraphase_core::PropertiesExt;
        let bad_data = b"not a font";
        let props = Properties::new().with("format", "ttf");
        let result = TtfToWoff.convert(bad_data, &props);
        assert!(result.is_err());
    }

    #[test]
    #[cfg(feature = "woff")]
    fn test_woff_to_ttf_bad_magic() {
        use super::*;
        use paraphase_core::PropertiesExt;
        let bad_data = b"not a woff file at all really";
        let props = Properties::new().with("format", "woff");
        let result = WoffToTtf.convert(bad_data, &props);
        assert!(result.is_err());
    }
}
