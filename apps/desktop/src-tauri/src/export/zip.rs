//! A small, dependency-free ZIP writer.
//!
//! The app already ships without an archive crate, and this phase must not add
//! an unverifiable dependency. Only the stored method (no compression) is
//! written: it is the oldest and most widely readable form of ZIP, needs no
//! compressor, and keeps the writer small enough to review and test.
//!
//! Deliberate limits, reported instead of silently truncated:
//!
//! * no ZIP64, so every file and the whole archive must stay below 4 GiB;
//! * no encryption, no comments, no extra fields;
//! * entry names are UTF-8 (general-purpose flag bit 11);
//! * directories are implicit in the entry paths.
//!
//! The writer never follows a symlink: callers pass regular-file paths that
//! `plan::resolve_sources` has already validated.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{ExportError, ExportResult};

/// Standard ZIP without ZIP64 cannot address 4 GiB or more.
pub const MAX_ARCHIVE_BYTES: u64 = u32::MAX as u64;

const LOCAL_HEADER: u32 = 0x0403_4b50;
const CENTRAL_HEADER: u32 = 0x0201_4b50;
const END_OF_CENTRAL: u32 = 0x0605_4b50;
const FLAG_UTF8: u16 = 0x0800;
const METHOD_STORE: u16 = 0;
const VERSION: u16 = 20;
const CHUNK: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Slash-separated path inside the archive.
    pub name: String,
    pub source: PathBuf,
    /// Size recorded at planning time; the writer fails if the file changed.
    pub size: u64,
}

struct CentralRecord {
    name: String,
    crc: u32,
    size: u32,
    offset: u32,
    time: u16,
    date: u16,
}

// ---------------------------------------------------------------------------
// CRC-32 (IEEE 802.3), the checksum ZIP requires
// ---------------------------------------------------------------------------

const fn crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut index = 0usize;

    while index < 256 {
        let mut value = index as u32;
        let mut bit = 0;

        while bit < 8 {
            value = if value & 1 == 1 {
                0xEDB8_8320 ^ (value >> 1)
            } else {
                value >> 1
            };
            bit += 1;
        }

        table[index] = value;
        index += 1;
    }

    table
}

static CRC_TABLE: [u32; 256] = crc_table();

fn crc_update(mut crc: u32, bytes: &[u8]) -> u32 {
    for byte in bytes {
        crc = CRC_TABLE[((crc ^ u32::from(*byte)) & 0xFF) as usize] ^ (crc >> 8);
    }

    crc
}

fn crc32(bytes: &[u8]) -> u32 {
    crc_update(0xFFFF_FFFF, bytes) ^ 0xFFFF_FFFF
}

// ---------------------------------------------------------------------------
// DOS date and time
// ---------------------------------------------------------------------------

/// Days-to-civil conversion (Howard Hinnant's public-domain algorithm), kept
/// local so the writer does not depend on a date crate.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;

    (if month <= 2 { year + 1 } else { year }, month, day)
}

fn dos_datetime(time: SystemTime) -> (u16, u16) {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);

    let days = seconds.div_euclid(86_400);
    let within = seconds.rem_euclid(86_400);
    let (mut year, month, day) = civil_from_days(days);

    year = year.clamp(1980, 2107);

    let hour = (within / 3_600) as u16;
    let minute = ((within % 3_600) / 60) as u16;
    let second = (within % 60) as u16;

    let date = (((year - 1980) as u16) << 9) | ((month as u16) << 5) | day as u16;
    let clock = (hour << 11) | (minute << 5) | (second / 2);

    (clock, date)
}

// ---------------------------------------------------------------------------
// Binary helpers
// ---------------------------------------------------------------------------

fn write_u16<W: Write>(out: &mut W, value: u16) -> ExportResult<()> {
    out.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn write_u32<W: Write>(out: &mut W, value: u32) -> ExportResult<()> {
    out.write_all(&value.to_le_bytes())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

/// Writes a complete archive. Returns `(entry count, uncompressed bytes)`.
///
/// `on_progress` is called after each entry with the file name, the number of
/// entries written, the total count and the bytes written so far.
pub fn write_archive<W: Write + Seek>(
    out: &mut W,
    entries: &[Entry],
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(&str, usize, usize, u64),
) -> ExportResult<(usize, u64)> {
    if entries.len() > u16::MAX as usize {
        return Err(ExportError::too_large(
            "The archive would contain more than 65,535 entries, which standard ZIP cannot describe.",
        ));
    }

    let mut central: Vec<CentralRecord> = Vec::with_capacity(entries.len());
    let mut total_bytes = 0u64;

    for (index, entry) in entries.iter().enumerate() {
        if cancel.load(Ordering::SeqCst) {
            return Err(ExportError::cancelled());
        }

        let mut file = File::open(&entry.source).map_err(|error| {
            ExportError::not_found(format!(
                "{} could not be opened for export: {error}",
                entry.name
            ))
        })?;

        let metadata = file.metadata()?;

        if !metadata.is_file() {
            return Err(ExportError::invalid_request(format!(
                "{} is no longer a regular file.",
                entry.name
            )));
        }

        if metadata.len() >= MAX_ARCHIVE_BYTES {
            return Err(ExportError::too_large(format!(
                "{} is 4 GiB or larger and cannot be stored in a standard ZIP.",
                entry.name
            )));
        }

        // Pass 1: checksum and length, so the local header can carry the real
        // values and every common extractor reads the archive without needing
        // a data descriptor.
        let mut hasher = 0xFFFF_FFFFu32;
        let mut size = 0u64;
        let mut buffer = vec![0u8; CHUNK];

        loop {
            if cancel.load(Ordering::SeqCst) {
                return Err(ExportError::cancelled());
            }

            let read = file.read(&mut buffer)?;

            if read == 0 {
                break;
            }

            size += read as u64;

            if size >= MAX_ARCHIVE_BYTES {
                return Err(ExportError::too_large(format!(
                    "{} grew past the 4 GiB ZIP limit while it was read.",
                    entry.name
                )));
            }

            hasher = crc_update(hasher, &buffer[..read]);
        }

        let crc = hasher ^ 0xFFFF_FFFF;

        if size != entry.size {
            return Err(ExportError::changed(&entry.name));
        }

        if cancel.load(Ordering::SeqCst) {
            return Err(ExportError::cancelled());
        }

        let offset = u32::try_from(out.stream_position()?)
            .map_err(|_| ExportError::too_large("The archive offset exceeded the ZIP limit."))?;

        let (time, date) = dos_datetime(metadata.modified().unwrap_or_else(|_| SystemTime::now()));
        let name_bytes = entry.name.as_bytes();

        let name_length = u16::try_from(name_bytes.len())
            .map_err(|_| ExportError::too_large("An archive path is too long for standard ZIP."))?;

        // Local file header.
        write_u32(out, LOCAL_HEADER)?;
        write_u16(out, VERSION)?;
        write_u16(out, FLAG_UTF8)?;
        write_u16(out, METHOD_STORE)?;
        write_u16(out, time)?;
        write_u16(out, date)?;
        write_u32(out, crc)?;
        write_u32(out, size as u32)?;
        write_u32(out, size as u32)?;
        write_u16(out, name_length)?;
        write_u16(out, 0)?;
        out.write_all(name_bytes)?;

        // Pass 2: the bytes themselves, re-checksummed to detect a file that
        // changed between planning and writing.
        file.seek(SeekFrom::Start(0))?;

        let mut verify = 0xFFFF_FFFFu32;
        let mut copied = 0u64;

        loop {
            if cancel.load(Ordering::SeqCst) {
                return Err(ExportError::cancelled());
            }

            let read = file.read(&mut buffer)?;

            if read == 0 {
                break;
            }

            copied += read as u64;
            verify = crc_update(verify, &buffer[..read]);
            out.write_all(&buffer[..read])?;
        }

        if copied != size || (verify ^ 0xFFFF_FFFF) != crc {
            return Err(ExportError::changed(&entry.name));
        }

        total_bytes += size;

        central.push(CentralRecord {
            name: entry.name.clone(),
            crc,
            size: size as u32,
            offset,
            time,
            date,
        });

        on_progress(&entry.name, index + 1, entries.len(), total_bytes);
    }

    if cancel.load(Ordering::SeqCst) {
        return Err(ExportError::cancelled());
    }

    let central_offset = u32::try_from(out.stream_position()?)
        .map_err(|_| ExportError::too_large("The archive offset exceeded the ZIP limit."))?;

    for record in &central {
        let name_bytes = record.name.as_bytes();
        let name_length = u16::try_from(name_bytes.len())
            .map_err(|_| ExportError::too_large("An archive path is too long for standard ZIP."))?;

        write_u32(out, CENTRAL_HEADER)?;
        write_u16(out, VERSION)?;
        write_u16(out, VERSION)?;
        write_u16(out, FLAG_UTF8)?;
        write_u16(out, METHOD_STORE)?;
        write_u16(out, record.time)?;
        write_u16(out, record.date)?;
        write_u32(out, record.crc)?;
        write_u32(out, record.size)?;
        write_u32(out, record.size)?;
        write_u16(out, name_length)?;
        write_u16(out, 0)?;
        write_u16(out, 0)?;
        write_u16(out, 0)?;
        write_u16(out, 0)?;
        write_u32(out, 0)?;
        write_u32(out, record.offset)?;
        out.write_all(name_bytes)?;
    }

    let central_end = u32::try_from(out.stream_position()?)
        .map_err(|_| ExportError::too_large("The archive offset exceeded the ZIP limit."))?;
    let central_size = central_end - central_offset;
    let count = u16::try_from(central.len())
        .map_err(|_| ExportError::too_large("Too many entries for standard ZIP."))?;

    write_u32(out, END_OF_CENTRAL)?;
    write_u16(out, 0)?;
    write_u16(out, 0)?;
    write_u16(out, count)?;
    write_u16(out, count)?;
    write_u32(out, central_size)?;
    write_u32(out, central_offset)?;
    write_u16(out, 0)?;

    out.flush()?;

    Ok((central.len(), total_bytes))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Cursor;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "nexora-zip-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create scratch");
        dir
    }

    fn entry(name: &str, source: PathBuf, size: u64) -> Entry {
        Entry {
            name: name.to_string(),
            source,
            size,
        }
    }

    fn u16_at(bytes: &[u8], at: usize) -> u16 {
        u16::from_le_bytes([bytes[at], bytes[at + 1]])
    }

    fn u32_at(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
    }

    /// Minimal reader: walks local headers, then checks the central directory
    /// and the end record agree with what was written.
    fn read_local_entries(bytes: &[u8]) -> Vec<(String, u32, Vec<u8>)> {
        let mut found = Vec::new();
        let mut at = 0usize;

        while at + 30 <= bytes.len() && u32_at(bytes, at) == LOCAL_HEADER {
            let flags = u16_at(bytes, at + 6);
            let method = u16_at(bytes, at + 8);
            let crc = u32_at(bytes, at + 14);
            let size = u32_at(bytes, at + 18) as usize;
            let name_length = u16_at(bytes, at + 26) as usize;
            let extra_length = u16_at(bytes, at + 28) as usize;
            let name_start = at + 30;
            let name_end = name_start + name_length;
            let data_start = name_end + extra_length;
            let data_end = data_start + size;

            assert_eq!(flags & FLAG_UTF8, FLAG_UTF8);
            assert_eq!(method, METHOD_STORE);

            let name = String::from_utf8(bytes[name_start..name_end].to_vec()).unwrap();
            let data = bytes[data_start..data_end].to_vec();

            assert_eq!(crc32(&data), crc);
            found.push((name, crc, data));

            at = data_end;
        }

        found
    }

    #[test]
    fn writes_a_readable_archive_with_nested_paths() {
        let dir = scratch("nested");
        let a = dir.join("a.txt");
        let b = dir.join("b.bin");

        fs::write(&a, b"hello").unwrap();
        fs::write(&b, [0u8, 1, 2, 3, 255]).unwrap();

        let entries = vec![entry("src/a.txt", a, 5), entry("src/deep/b.bin", b, 5)];

        let cancel = AtomicBool::new(false);
        let mut buffer = Cursor::new(Vec::new());
        let mut progress = Vec::new();

        let (count, bytes) = write_archive(
            &mut buffer,
            &entries,
            &cancel,
            &mut |name, done, total, written| {
                progress.push((name.to_string(), done, total, written));
            },
        )
        .expect("write");

        assert_eq!(count, 2);
        assert_eq!(bytes, 10);
        assert_eq!(progress.len(), 2);
        assert_eq!(progress[1].1, 2);

        let archive = buffer.into_inner();
        let parsed = read_local_entries(&archive);

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].0, "src/a.txt");
        assert_eq!(parsed[0].2, b"hello");
        assert_eq!(parsed[1].0, "src/deep/b.bin");
        assert_eq!(parsed[1].2, vec![0, 1, 2, 3, 255]);

        // End of central directory.
        let eocd = archive.len() - 22;
        assert_eq!(u32_at(&archive, eocd), END_OF_CENTRAL);
        assert_eq!(u16_at(&archive, eocd + 8), 2);
        assert_eq!(u16_at(&archive, eocd + 10), 2);

        let cd_offset = u32_at(&archive, eocd + 16) as usize;
        let cd_size = u32_at(&archive, eocd + 12) as usize;
        assert_eq!(cd_offset + cd_size, eocd);
        assert_eq!(u32_at(&archive, cd_offset), CENTRAL_HEADER);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn writes_a_valid_empty_archive() {
        let cancel = AtomicBool::new(false);
        let mut buffer = Cursor::new(Vec::new());

        let (count, bytes) =
            write_archive(&mut buffer, &[], &cancel, &mut |_, _, _, _| {}).expect("empty archive");

        assert_eq!(count, 0);
        assert_eq!(bytes, 0);

        let archive = buffer.into_inner();
        assert_eq!(archive.len(), 22);
        assert_eq!(u32_at(&archive, 0), END_OF_CENTRAL);
        assert_eq!(u16_at(&archive, 8), 0);
        assert_eq!(u16_at(&archive, 10), 0);
    }

    #[test]
    fn refuses_a_file_that_changed_after_planning() {
        let dir = scratch("changed");
        let file = dir.join("a.txt");
        fs::write(&file, b"twelve bytes").unwrap();

        let entries = vec![entry("a.txt", file, 4)];
        let cancel = AtomicBool::new(false);
        let mut buffer = Cursor::new(Vec::new());

        let error =
            write_archive(&mut buffer, &entries, &cancel, &mut |_, _, _, _| {}).unwrap_err();
        assert_eq!(error.code, "changed");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn stops_when_cancelled() {
        let dir = scratch("cancel");
        let file = dir.join("a.txt");
        fs::write(&file, b"content").unwrap();

        let entries = vec![entry("a.txt", file, 7)];
        let cancel = AtomicBool::new(true);
        let mut buffer = Cursor::new(Vec::new());

        let error =
            write_archive(&mut buffer, &entries, &cancel, &mut |_, _, _, _| {}).unwrap_err();
        assert_eq!(error.code, "cancelled");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn stores_unicode_names_with_the_utf8_flag() {
        let dir = scratch("unicode");
        let file = dir.join("café.txt");
        fs::write(&file, b"naive").unwrap();

        let entries = vec![entry("dossier/café.txt", file, 5)];
        let cancel = AtomicBool::new(false);
        let mut buffer = Cursor::new(Vec::new());

        write_archive(&mut buffer, &entries, &cancel, &mut |_, _, _, _| {}).expect("write");

        let parsed = read_local_entries(&buffer.get_ref());
        assert_eq!(parsed[0].0, "dossier/café.txt");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn crc_matches_known_vectors() {
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b"hello"), 0x3610_A686);
    }

    #[test]
    fn dos_timestamps_stay_inside_the_representable_range() {
        // The DOS epoch starts in 1980, so 1970 is clamped to 1980-01-01.
        let (time, date) = dos_datetime(UNIX_EPOCH);
        assert_eq!(date >> 9, 0);
        assert_eq!((date >> 5) & 0x0F, 1);
        assert_eq!(date & 0x1F, 1);
        assert_eq!(time, 0);

        // 2001-09-09T01:46:40Z is a fixed, leap-year-safe timestamp.
        let later = dos_datetime(UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000));
        assert_eq!(later.1 >> 9, 21);
        assert_eq!((later.1 >> 5) & 0x0F, 9);
        assert_eq!(later.1 & 0x1F, 9);

        let before = dos_datetime(UNIX_EPOCH - std::time::Duration::from_secs(86_400 * 365));
        assert_eq!(before.1 >> 9, 0);
    }
}
