//! Adapter for the readsb `aircraft.json` format.
//!
//! readsb (https://github.com/wiedehopf/readsb) is the common decoder for an
//! owned 1090 MHz receiver. Its `aircraft.json` has a top-level `now` in
//! seconds and an `aircraft` array. Fields are frequently absent when a given
//! message type has not been received recently; we map every absence to an
//! explicit `None` rather than a zero.
//!
//! Schema handling (A01): readsb does not carry an explicit schema version in
//! the file, so we treat the presence of `now` and `aircraft` as the v1 shape
//! and reject anything else with a descriptive error instead of guessing.

use aero_geo::Geodetic;
use aero_types::{
    BaroAltM, Callsign, GeoAltM, GroundState, Icao24, IngestError, IngestWallTime, MaybeTime,
    Observation, ObservationTime, SourceId, SpeedMps, TrueTrackDeg, VerticalRateMps,
};
use serde::Deserialize;

/// Maximum accepted size of a single readsb document, to bound memory against a
/// hostile or broken feed (A01, A11 backpressure at the edge).
pub const MAX_DOC_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct ReadsbDoc {
    now: f64,
    aircraft: Vec<ReadsbAircraft>,
}

/// readsb altitude fields are either a number (feet) or the string "ground".
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum AltField {
    Feet(f64),
    Tag(String),
}

#[derive(Debug, Deserialize, Default)]
struct ReadsbAircraft {
    hex: String,
    #[serde(default)]
    flight: Option<String>,
    #[serde(default)]
    lat: Option<f64>,
    #[serde(default)]
    lon: Option<f64>,
    #[serde(default)]
    alt_baro: Option<AltField>,
    #[serde(default)]
    alt_geom: Option<f64>,
    #[serde(default)]
    gs: Option<f64>,
    #[serde(default)]
    track: Option<f64>,
    #[serde(default)]
    baro_rate: Option<f64>,
    #[serde(default)]
    geom_rate: Option<f64>,
    /// Seconds since this aircraft's position was last updated.
    #[serde(default)]
    seen_pos: Option<f64>,
    /// Seconds since any message from this aircraft.
    #[serde(default)]
    seen: Option<f64>,
    #[serde(default)]
    messages: Option<u64>,
}

/// Parse a readsb document into normalized observations. The `source` names the
/// receiver so attribution survives fusion. Malformed aircraft entries are
/// returned as errors for quarantine; a single bad entry does not fail the doc.
pub fn parse_readsb(bytes: &[u8], source: &SourceId) -> Result<ReadsbParse, IngestError> {
    if bytes.len() > MAX_DOC_BYTES {
        return Err(IngestError::OversizedRecord {
            limit: MAX_DOC_BYTES,
        });
    }
    let doc: ReadsbDoc = serde_json::from_slice(bytes)
        .map_err(|e| IngestError::Malformed(format!("readsb doc: {e}")))?;

    // readsb `now` is a Unix time in seconds (float). Convert to millis once.
    let now_ms = (doc.now * 1000.0).round() as i64;

    let mut observations = Vec::with_capacity(doc.aircraft.len());
    let mut rejected = Vec::new();

    for ac in doc.aircraft {
        match normalize_aircraft(&ac, now_ms, source) {
            Ok(obs) => observations.push(obs),
            Err(e) => rejected.push((ac.hex.clone(), e.to_string())),
        }
    }
    Ok(ReadsbParse {
        observations,
        rejected,
    })
}

/// Result of parsing one readsb document.
#[derive(Debug)]
pub struct ReadsbParse {
    pub observations: Vec<Observation>,
    /// (hex, reason) pairs for entries that failed normalization.
    pub rejected: Vec<(String, String)>,
}

fn normalize_aircraft(
    ac: &ReadsbAircraft,
    now_ms: i64,
    source: &SourceId,
) -> Result<Observation, IngestError> {
    let icao24 = Icao24::parse_hex(&ac.hex)
        .ok_or_else(|| IngestError::Malformed(format!("bad hex '{}'", ac.hex)))?;
    if icao24.is_reserved() {
        return Err(IngestError::Malformed("reserved icao24".into()));
    }

    // Reconstruct the per-aircraft observation time from `now` minus `seen_pos`
    // (for the position) or `seen`. readsb does not give an absolute per-field
    // timestamp, so this is the best honest reconstruction; the basis is the
    // receiver clock and is documented as such.
    let seen_s = ac.seen_pos.or(ac.seen);
    let observation_time = match seen_s {
        Some(s) if s.is_finite() && s >= 0.0 => {
            MaybeTime::Known(ObservationTime(now_ms - (s * 1000.0).round() as i64))
        }
        _ => MaybeTime::Absent,
    };

    // Ground vs airborne comes from alt_baro == "ground".
    let (ground_state, baro_from_tag) = match &ac.alt_baro {
        Some(AltField::Tag(t)) if t == "ground" => (GroundState::OnGround, true),
        Some(AltField::Feet(_)) => (GroundState::Airborne, false),
        _ => (GroundState::Unknown, false),
    };

    let baro_alt = match &ac.alt_baro {
        Some(AltField::Feet(ft)) => {
            check_range("alt_baro", *ft, -2000.0, 70000.0)?;
            Some(BaroAltM::from_feet(*ft))
        }
        _ => None,
    };
    let geo_alt = match ac.alt_geom {
        Some(ft) => {
            check_range("alt_geom", ft, -2000.0, 70000.0)?;
            Some(GeoAltM::from_feet(ft))
        }
        None => None,
    };

    // Position requires both lat and lon; one without the other is dropped to
    // None rather than fabricated.
    let position = match (ac.lat, ac.lon) {
        (Some(lat), Some(lon)) => {
            check_range("lat", lat, -90.0, 90.0)?;
            check_range("lon", lon, -180.0, 180.0)?;
            // Use geometric altitude for placement when available; otherwise 0
            // and flag via geo_alt absence. We never place with baro altitude.
            let alt = geo_alt.map(|g| g.0).unwrap_or(0.0);
            Some(Geodetic::new(lat, lon, alt))
        }
        _ => None,
    };

    let ground_speed = ac.gs.and_then(|kt| {
        if (0.0..=1200.0).contains(&kt) {
            Some(SpeedMps::from_knots(kt))
        } else {
            None
        }
    });
    let true_track = ac.track.and_then(|t| {
        if (0.0..360.0).contains(&t) {
            Some(TrueTrackDeg(t))
        } else {
            None
        }
    });
    // Prefer geometric vertical rate when present, else barometric.
    let vertical_rate = ac.geom_rate.or(ac.baro_rate).map(VerticalRateMps::from_fpm);

    let _ = baro_from_tag; // documented above; kept for clarity of branches

    Ok(Observation {
        source: source.clone(),
        icao24,
        callsign: ac.flight.as_deref().and_then(Callsign::normalize),
        observation_time,
        publish_time: MaybeTime::Absent, // readsb has no upstream publish time
        ingest_wall_time: IngestWallTime(now_ms),
        position,
        baro_alt,
        geo_alt,
        ground_speed,
        true_track,
        vertical_rate,
        ground_state,
        source_sequence: ac.messages, // message counter acts as a gap detector
    })
}

fn check_range(field: &'static str, v: f64, lo: f64, hi: f64) -> Result<(), IngestError> {
    if !v.is_finite() || v < lo || v > hi {
        return Err(IngestError::OutOfRange {
            field,
            detail: format!("{v}"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "now": 1730000000.0,
      "aircraft": [
        {"hex":"4ca7b3","flight":"DLH42  ","lat":51.47,"lon":-0.45,
         "alt_baro":35000,"alt_geom":35050,"gs":450,"track":270.0,
         "geom_rate":-64,"seen_pos":0.4,"seen":0.4,"messages":812},
        {"hex":"a1b2c3","alt_baro":"ground","lat":40.64,"lon":-73.78,
         "gs":12,"track":90.0,"seen":1.0},
        {"hex":"zzzzzz","lat":1.0,"lon":2.0}
      ]
    }"#;

    #[test]
    fn parses_valid_aircraft_and_quarantines_bad_hex() {
        let src = SourceId("rx-home".into());
        let p = parse_readsb(SAMPLE.as_bytes(), &src).unwrap();
        assert_eq!(p.observations.len(), 2);
        assert_eq!(p.rejected.len(), 1);
        assert_eq!(p.rejected[0].0, "zzzzzz");
    }

    #[test]
    fn airborne_entry_has_position_and_both_altitudes() {
        let src = SourceId("rx-home".into());
        let p = parse_readsb(SAMPLE.as_bytes(), &src).unwrap();
        let a = &p.observations[0];
        assert_eq!(a.icao24.to_hex(), "4ca7b3");
        assert_eq!(a.callsign.as_ref().unwrap().0, "DLH42");
        assert!(a.position.is_some());
        assert!(a.baro_alt.is_some());
        assert!(a.geo_alt.is_some());
        // Placement uses geometric altitude, not barometric.
        let pos = a.position.unwrap();
        assert!((pos.alt_m - a.geo_alt.unwrap().0).abs() < 1e-6);
        assert_eq!(a.ground_state, GroundState::Airborne);
        // Vertical rate is a descent.
        assert!(a.vertical_rate.unwrap().0 < 0.0);
    }

    #[test]
    fn ground_entry_marked_on_ground_without_baro_altitude() {
        let src = SourceId("rx-home".into());
        let p = parse_readsb(SAMPLE.as_bytes(), &src).unwrap();
        let g = &p.observations[1];
        assert_eq!(g.ground_state, GroundState::OnGround);
        assert!(g.baro_alt.is_none());
        assert!(g.position.is_some()); // ground vehicles/taxi still have a position
    }

    #[test]
    fn observation_time_reconstructed_from_seen_pos() {
        let src = SourceId("rx-home".into());
        let p = parse_readsb(SAMPLE.as_bytes(), &src).unwrap();
        let a = &p.observations[0];
        // now = 1730000000.0s, seen_pos = 0.4s -> observation 400ms earlier.
        let (t, _) = a.effective_event_time_ms();
        assert_eq!(t, 1_730_000_000_000 - 400);
    }

    #[test]
    fn oversized_document_rejected() {
        let src = SourceId("rx".into());
        let big = vec![b' '; MAX_DOC_BYTES + 1];
        assert!(matches!(
            parse_readsb(&big, &src),
            Err(IngestError::OversizedRecord { .. })
        ));
    }

    #[test]
    fn malformed_json_is_an_error_not_a_panic() {
        let src = SourceId("rx".into());
        assert!(parse_readsb(b"{not json", &src).is_err());
    }
}
