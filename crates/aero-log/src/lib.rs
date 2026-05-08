//! Durable append-only observation log (A04).
//!
//! The log is the system's source of truth: every accepted observation is
//! framed and appended before fusion sees it, so a crash loses at most the
//! in-flight record and recovery can reconstruct exactly what was accepted.
//!
//! Record frame on disk (all integers little-endian):
//! ```text
//!   magic u32 = 0xAE05_0B01
//!   len   u32  (length of the JSON payload in bytes)
//!   crc   u32  (CRC-32 of the payload only)
//!   payload[len]  (serde_json of the stored record)
//! ```
//! Recovery (A04 restart recovery): on open we scan frames from the start; a
//! frame whose magic, length, or CRC does not validate marks the end of the
//! intact log and the file is truncated there, discarding a torn tail from a
//! crash mid-append. Earlier records are kept.
//!
//! Idempotent ingestion (A04): each stored record carries a dedup key
//! (source, icao24, event-time, event-time basis). The in-memory index rejects
//! a second append with the same key so replaying a source does not duplicate.

mod crc32;
pub mod record;

pub use record::{LoggedObservation, DedupKey};

use crc32::crc32;
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const MAGIC: u32 = 0xAE05_0B01;
const HEADER_LEN: usize = 12; // magic + len + crc
/// Bounded maximum payload to reject an absurd length field from a torn frame.
const MAX_PAYLOAD: u32 = 4 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum LogError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("duplicate observation ignored")]
    Duplicate,
}

/// The append-only log with an in-memory dedup index and byte offset tracking.
pub struct ObservationLog {
    path: PathBuf,
    file: File,
    /// Byte offset of the next append == current intact length.
    next_offset: u64,
    /// Number of intact records recovered/appended.
    count: u64,
    seen: HashSet<DedupKey>,
}

impl ObservationLog {
    /// Open (creating if absent) and recover to the last intact record.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, LogError> {
        let path = path.as_ref().to_path_buf();
        let intact_len = Self::recover_intact_len(&path)?;

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)?;
        // Truncate any torn tail beyond the last intact record.
        file.set_len(intact_len)?;

        let mut log = Self {
            path,
            file,
            next_offset: intact_len,
            count: 0,
            seen: HashSet::new(),
        };
        log.rebuild_index()?;
        log.file.seek(SeekFrom::End(0))?;
        Ok(log)
    }

    /// Append one observation. Returns its byte offset, or Duplicate if the
    /// dedup key was already present (idempotent ingestion).
    pub fn append(&mut self, obs: &LoggedObservation) -> Result<u64, LogError> {
        let key = obs.dedup_key();
        if self.seen.contains(&key) {
            return Err(LogError::Duplicate);
        }
        let payload = serde_json::to_vec(obs)?;
        let len = payload.len() as u32;
        let crc = crc32(&payload);

        let mut frame = Vec::with_capacity(HEADER_LEN + payload.len());
        frame.extend_from_slice(&MAGIC.to_le_bytes());
        frame.extend_from_slice(&len.to_le_bytes());
        frame.extend_from_slice(&crc.to_le_bytes());
        frame.extend_from_slice(&payload);

        self.file.write_all(&frame)?;
        self.file.flush()?;

        let offset = self.next_offset;
        self.next_offset += frame.len() as u64;
        self.count += 1;
        self.seen.insert(key);
        Ok(offset)
    }

    pub fn count(&self) -> u64 {
        self.count
    }
    pub fn len_bytes(&self) -> u64 {
        self.next_offset
    }

    /// Read every intact record back in append order.
    pub fn read_all(&self) -> Result<Vec<LoggedObservation>, LogError> {
        let f = File::open(&self.path)?;
        let mut reader = BufReader::new(f);
        let mut out = Vec::new();
        loop {
            match Self::read_frame(&mut reader)? {
                Some(payload) => {
                    let obs: LoggedObservation = serde_json::from_slice(&payload)?;
                    out.push(obs);
                }
                None => break,
            }
        }
        Ok(out)
    }

    fn rebuild_index(&mut self) -> Result<(), LogError> {
        for obs in self.read_all()? {
            self.seen.insert(obs.dedup_key());
            self.count += 1;
        }
        Ok(())
    }

    /// Scan from the start; return the byte length of the contiguous run of
    /// valid frames. A bad header or CRC ends the intact region.
    fn recover_intact_len(path: &Path) -> Result<u64, LogError> {
        if !path.exists() {
            return Ok(0);
        }
        let f = File::open(path)?;
        let mut reader = BufReader::new(f);
        let mut intact: u64 = 0;
        loop {
            let frame_len = match Self::probe_frame(&mut reader)? {
                Some(n) => n,
                None => break,
            };
            intact += frame_len as u64;
        }
        Ok(intact)
    }

    /// Read a frame returning its payload, or None at clean EOF. A corrupt or
    /// truncated frame returns None (caller treats it as end of intact log).
    fn read_frame<R: Read>(reader: &mut R) -> Result<Option<Vec<u8>>, LogError> {
        let mut header = [0u8; HEADER_LEN];
        match reader.read_exact(&mut header) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e.into()),
        }
        let magic = u32::from_le_bytes(header[0..4].try_into().unwrap());
        let len = u32::from_le_bytes(header[4..8].try_into().unwrap());
        let crc = u32::from_le_bytes(header[8..12].try_into().unwrap());
        if magic != MAGIC || len == 0 || len > MAX_PAYLOAD {
            return Ok(None);
        }
        let mut payload = vec![0u8; len as usize];
        if reader.read_exact(&mut payload).is_err() {
            return Ok(None); // truncated tail
        }
        if crc32(&payload) != crc {
            return Ok(None); // corrupt record ends intact region
        }
        Ok(Some(payload))
    }

    /// Like read_frame but returns the total on-disk frame length, for the
    /// recovery scan that needs byte counts rather than payloads.
    fn probe_frame<R: Read>(reader: &mut R) -> Result<Option<usize>, LogError> {
        match Self::read_frame(reader)? {
            Some(payload) => Ok(Some(HEADER_LEN + payload.len())),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aero_geo::Geodetic;
    use aero_types::*;

    fn obs(seq: u64, t: i64) -> LoggedObservation {
        let o = Observation {
            source: SourceId("rx".into()),
            icao24: Icao24::parse_hex("4ca7b3").unwrap(),
            callsign: Callsign::normalize("DLH42"),
            observation_time: MaybeTime::Known(ObservationTime(t)),
            publish_time: MaybeTime::Absent,
            ingest_wall_time: IngestWallTime(t + 100),
            position: Some(Geodetic::new(51.0, 0.0, 10000.0)),
            baro_alt: Some(BaroAltM(10000.0)),
            geo_alt: Some(GeoAltM(10050.0)),
            ground_speed: Some(SpeedMps(230.0)),
            true_track: Some(TrueTrackDeg(270.0)),
            vertical_rate: Some(VerticalRateMps(0.0)),
            ground_state: GroundState::Airborne,
            source_sequence: Some(seq),
        };
        LoggedObservation::new(o)
    }

    #[test]
    fn append_and_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.aero");
        let mut log = ObservationLog::open(&path).unwrap();
        log.append(&obs(1, 1000)).unwrap();
        log.append(&obs(2, 2000)).unwrap();
        assert_eq!(log.count(), 2);
        let all = log.read_all().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[1].observation.source_sequence, Some(2));
    }

    #[test]
    fn idempotent_append_rejects_duplicate_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.aero");
        let mut log = ObservationLog::open(&path).unwrap();
        log.append(&obs(1, 1000)).unwrap();
        // Same source/icao/event-time/basis -> duplicate.
        let err = log.append(&obs(1, 1000)).unwrap_err();
        assert!(matches!(err, LogError::Duplicate));
        assert_eq!(log.count(), 1);
    }

    #[test]
    fn recovery_reopens_and_preserves_records() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.aero");
        {
            let mut log = ObservationLog::open(&path).unwrap();
            log.append(&obs(1, 1000)).unwrap();
            log.append(&obs(2, 2000)).unwrap();
        }
        let log = ObservationLog::open(&path).unwrap();
        assert_eq!(log.count(), 2);
        // Dedup index rebuilt: a reopened log still rejects a known key.
        let mut log = log;
        assert!(matches!(log.append(&obs(1, 1000)), Err(LogError::Duplicate)));
    }

    #[test]
    fn torn_tail_is_truncated_on_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.aero");
        {
            let mut log = ObservationLog::open(&path).unwrap();
            log.append(&obs(1, 1000)).unwrap();
            log.append(&obs(2, 2000)).unwrap();
        }
        // Simulate a crash mid-append: append garbage bytes to the file.
        {
            let mut f = OpenOptions::new().append(true).open(&path).unwrap();
            f.write_all(&MAGIC.to_le_bytes()).unwrap();
            f.write_all(&[0xFF, 0xFF]).unwrap(); // partial header
        }
        let log = ObservationLog::open(&path).unwrap();
        // Torn tail discarded; the two good records survive.
        assert_eq!(log.count(), 2);
        assert_eq!(log.read_all().unwrap().len(), 2);
    }
}
