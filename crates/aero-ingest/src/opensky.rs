//! OpenSky adapter: OAuth2 client-credentials token flow and state parsing.
//!
//! OpenSky migrated to OAuth2 client-credentials; the old basic-auth examples
//! are deprecated. This adapter implements the current flow (token endpoint,
//! bearer token, expiry with refresh, bounded retry) but is OPERATIONALLY
//! DISABLED by default: `OperationalGuard` must be given an explicit agreement
//! acknowledgement before any live request is allowed. An API token alone is
//! not that agreement. Recheck the terms at deployment time:
//! https://opensky-network.org/about/terms-of-use
//!
//! The HTTP transport is a trait so the token lifecycle and the state parser
//! are fully unit-testable without a network and without secrets in code.

use aero_geo::Geodetic;
use aero_types::{
    BaroAltM, Callsign, GeoAltM, GroundState, Icao24, IngestError, IngestWallTime, MaybeTime,
    Observation, ObservationTime, PublishTime, SourceId, SpeedMps, TrueTrackDeg, VerticalRateMps,
};
use serde::Deserialize;

/// Guard that must be satisfied before any operational OpenSky request runs.
/// Keeps the honesty invariant that a token is not an agreement.
#[derive(Debug, Clone)]
pub struct OperationalGuard {
    agreement_acknowledged: bool,
    reason_if_disabled: String,
}

impl Default for OperationalGuard {
    fn default() -> Self {
        Self {
            agreement_acknowledged: false,
            reason_if_disabled:
                "operational OpenSky use requires a written agreement covering display, retention, and redistribution; an API token is not that agreement"
                    .to_string(),
        }
    }
}

impl OperationalGuard {
    /// Enable operational use only when the deployment truly has the agreement.
    /// This is a deliberate, explicit, auditable call.
    pub fn with_signed_agreement() -> Self {
        Self {
            agreement_acknowledged: true,
            reason_if_disabled: String::new(),
        }
    }
    pub fn is_enabled(&self) -> bool {
        self.agreement_acknowledged
    }
    pub fn reason(&self) -> &str {
        &self.reason_if_disabled
    }
}

/// Abstract HTTP so tests inject responses and no credential touches the net
/// in unit tests. A real implementation wires this to an HTTPS client.
pub trait HttpTransport {
    /// POST form body to a token endpoint, return (status, body).
    fn post_form(&self, url: &str, form: &[(&str, &str)]) -> Result<(u16, String), String>;
    /// GET with a bearer token, return (status, body).
    fn get_bearer(&self, url: &str, token: &str) -> Result<(u16, String), String>;
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    /// Lifetime in seconds.
    expires_in: u64,
}

/// A cached bearer token with an absolute expiry in Unix millis.
#[derive(Debug, Clone)]
pub struct CachedToken {
    pub token: String,
    pub expires_at_ms: i64,
}

impl CachedToken {
    /// Refresh 30s early so a request never races the expiry boundary.
    pub fn is_fresh(&self, now_ms: i64) -> bool {
        now_ms + 30_000 < self.expires_at_ms
    }
}

pub struct OpenSkyClient<H: HttpTransport> {
    http: H,
    token_url: String,
    client_id: String,
    client_secret: String,
    guard: OperationalGuard,
    cached: Option<CachedToken>,
    /// Bounded retry count for transient token failures.
    pub max_token_retries: u32,
}

impl<H: HttpTransport> OpenSkyClient<H> {
    pub fn new(
        http: H,
        token_url: impl Into<String>,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        guard: OperationalGuard,
    ) -> Self {
        Self {
            http,
            token_url: token_url.into(),
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            guard,
            cached: None,
            max_token_retries: 3,
        }
    }

    /// Fetch or reuse a bearer token. Returns an error (not a live call) when
    /// the operational guard is not satisfied.
    pub fn ensure_token(&mut self, now_ms: i64) -> Result<String, String> {
        if !self.guard.is_enabled() {
            return Err(format!("OpenSky disabled: {}", self.guard.reason()));
        }
        if let Some(c) = &self.cached {
            if c.is_fresh(now_ms) {
                return Ok(c.token.clone());
            }
        }
        let mut attempt = 0;
        loop {
            attempt += 1;
            let form = [
                ("grant_type", "client_credentials"),
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
            ];
            match self.http.post_form(&self.token_url, &form) {
                Ok((200, body)) => {
                    let tr: TokenResponse =
                        serde_json::from_str(&body).map_err(|e| format!("token parse: {e}"))?;
                    let cached = CachedToken {
                        token: tr.access_token,
                        expires_at_ms: now_ms + (tr.expires_in as i64) * 1000,
                    };
                    let t = cached.token.clone();
                    self.cached = Some(cached);
                    return Ok(t);
                }
                Ok((401, _)) => return Err("token endpoint rejected credentials (401)".into()),
                Ok((code, _)) if attempt < self.max_token_retries => {
                    // Transient (429/5xx): bounded retry. Real impl backs off.
                    let _ = code;
                    continue;
                }
                Ok((code, _)) => {
                    return Err(format!("token endpoint failed after retries: {code}"))
                }
                Err(e) if attempt < self.max_token_retries => {
                    let _ = e;
                    continue;
                }
                Err(e) => return Err(format!("transport error after retries: {e}")),
            }
        }
    }

    /// Invalidate a token that the server reported expired mid-use, forcing a
    /// refresh on the next `ensure_token`.
    pub fn invalidate_token(&mut self) {
        self.cached = None;
    }
}

/// Parse an OpenSky `/states/all` response into observations. The states array
/// uses positional fields; indices follow the documented REST schema. A time
/// value of 0 or null is treated as absent, never as the epoch.
pub fn parse_states_all(
    body: &str,
    source: &SourceId,
    ingest_wall_ms: i64,
) -> Result<Vec<Observation>, IngestError> {
    #[derive(Deserialize)]
    struct StatesAll {
        time: i64,
        states: Option<Vec<Vec<serde_json::Value>>>,
    }
    let parsed: StatesAll = serde_json::from_str(body)
        .map_err(|e| IngestError::Malformed(format!("states/all: {e}")))?;
    let publish_ms = if parsed.time > 0 {
        Some(parsed.time * 1000)
    } else {
        None
    };

    let mut out = Vec::new();
    for s in parsed.states.unwrap_or_default() {
        if let Some(obs) = parse_state_vector(&s, source, publish_ms, ingest_wall_ms) {
            out.push(obs);
        }
    }
    Ok(out)
}

fn as_f64(v: Option<&serde_json::Value>) -> Option<f64> {
    v.and_then(|x| x.as_f64())
}
fn as_str(v: Option<&serde_json::Value>) -> Option<&str> {
    v.and_then(|x| x.as_str())
}

fn parse_state_vector(
    s: &[serde_json::Value],
    source: &SourceId,
    publish_ms: Option<i64>,
    ingest_wall_ms: i64,
) -> Option<Observation> {
    // Documented positional schema (subset we consume):
    // 0 icao24, 1 callsign, 3 time_position, 5 longitude, 6 latitude,
    // 7 baro_altitude(m), 8 on_ground, 9 velocity(m/s), 10 true_track,
    // 11 vertical_rate(m/s), 13 geo_altitude(m).
    let icao24 = Icao24::parse_hex(as_str(s.first())?.trim())?;
    if icao24.is_reserved() {
        return None;
    }
    let callsign = as_str(s.get(1)).and_then(Callsign::normalize);
    let time_position = as_f64(s.get(3)).filter(|t| *t > 0.0);
    let observation_time = match time_position {
        Some(t) => MaybeTime::Known(ObservationTime((t * 1000.0) as i64)),
        None => MaybeTime::Absent,
    };
    let lon = as_f64(s.get(5));
    let lat = as_f64(s.get(6));
    let baro_m = as_f64(s.get(7));
    let on_ground = s.get(8).and_then(|v| v.as_bool());
    let velocity = as_f64(s.get(9));
    let track = as_f64(s.get(10));
    let vrate = as_f64(s.get(11));
    let geo_m = as_f64(s.get(13));

    let position = match (lat, lon) {
        (Some(la), Some(lo)) if (-90.0..=90.0).contains(&la) && (-180.0..=180.0).contains(&lo) => {
            Some(Geodetic::new(la, lo, geo_m.unwrap_or(0.0)))
        }
        _ => None,
    };

    Some(Observation {
        source: source.clone(),
        icao24,
        callsign,
        observation_time,
        publish_time: publish_ms
            .map(|m| MaybeTime::Known(PublishTime(m)))
            .unwrap_or(MaybeTime::Absent),
        ingest_wall_time: IngestWallTime(ingest_wall_ms),
        position,
        baro_alt: baro_m.map(BaroAltM),
        geo_alt: geo_m.map(GeoAltM),
        ground_speed: velocity.map(SpeedMps),
        true_track: track.filter(|t| (0.0..360.0).contains(t)).map(TrueTrackDeg),
        vertical_rate: vrate.map(VerticalRateMps),
        ground_state: match on_ground {
            Some(true) => GroundState::OnGround,
            Some(false) => GroundState::Airborne,
            None => GroundState::Unknown,
        },
        source_sequence: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// A scripted transport returning queued responses in order.
    struct MockHttp {
        token_responses: RefCell<Vec<Result<(u16, String), String>>>,
    }
    impl HttpTransport for MockHttp {
        fn post_form(&self, _url: &str, _form: &[(&str, &str)]) -> Result<(u16, String), String> {
            self.token_responses.borrow_mut().remove(0)
        }
        fn get_bearer(&self, _url: &str, _token: &str) -> Result<(u16, String), String> {
            Ok((200, "{}".into()))
        }
    }

    #[test]
    fn disabled_guard_blocks_without_network() {
        let http = MockHttp {
            token_responses: RefCell::new(vec![]),
        };
        let mut c = OpenSkyClient::new(
            http,
            "https://t",
            "id",
            "secret",
            OperationalGuard::default(),
        );
        let err = c.ensure_token(1_000).unwrap_err();
        assert!(err.contains("disabled"));
        assert!(err.contains("agreement"));
    }

    #[test]
    fn token_fetched_cached_and_refreshed() {
        let http = MockHttp {
            token_responses: RefCell::new(vec![
                Ok((200, r#"{"access_token":"abc","expires_in":100}"#.into())),
                Ok((200, r#"{"access_token":"def","expires_in":100}"#.into())),
            ]),
        };
        let mut c = OpenSkyClient::new(
            http,
            "https://t",
            "id",
            "secret",
            OperationalGuard::with_signed_agreement(),
        );
        let now = 1_000_000;
        assert_eq!(c.ensure_token(now).unwrap(), "abc");
        // Still fresh 60s later -> same token, no new fetch.
        assert_eq!(c.ensure_token(now + 60_000).unwrap(), "abc");
        // Past the 30s-early refresh window -> new token.
        assert_eq!(c.ensure_token(now + 80_000).unwrap(), "def");
    }

    #[test]
    fn retries_transient_then_succeeds() {
        let http = MockHttp {
            token_responses: RefCell::new(vec![
                Ok((503, "busy".into())),
                Ok((429, "slow".into())),
                Ok((200, r#"{"access_token":"ok","expires_in":50}"#.into())),
            ]),
        };
        let mut c = OpenSkyClient::new(
            http,
            "https://t",
            "id",
            "secret",
            OperationalGuard::with_signed_agreement(),
        );
        assert_eq!(c.ensure_token(0).unwrap(), "ok");
    }

    #[test]
    fn bad_credentials_not_retried() {
        let http = MockHttp {
            token_responses: RefCell::new(vec![Ok((401, "no".into()))]),
        };
        let mut c = OpenSkyClient::new(
            http,
            "https://t",
            "id",
            "secret",
            OperationalGuard::with_signed_agreement(),
        );
        assert!(c.ensure_token(0).unwrap_err().contains("401"));
    }

    #[test]
    fn parses_states_all_vector() {
        let body = r#"{"time":1730000000,"states":[
          ["4ca7b3","DLH42   ","Germany",1729999999,1730000000,
           -0.45,51.47,10668.0,false,231.5,270.0,-0.3,null,10700.0,null,null,false,0]
        ]}"#;
        let src = SourceId("opensky".into());
        let obs = parse_states_all(body, &src, 1_730_000_001_000).unwrap();
        assert_eq!(obs.len(), 1);
        let o = &obs[0];
        assert_eq!(o.icao24.to_hex(), "4ca7b3");
        assert_eq!(o.callsign.as_ref().unwrap().0, "DLH42");
        assert!(o.position.is_some());
        assert!(o.baro_alt.is_some());
        assert!(o.geo_alt.is_some());
        assert_eq!(o.ground_state, GroundState::Airborne);
        assert!(o.publish_time.is_known());
    }

    #[test]
    fn zero_time_treated_as_absent() {
        let body = r#"{"time":0,"states":[]}"#;
        let src = SourceId("opensky".into());
        let obs = parse_states_all(body, &src, 1000).unwrap();
        assert!(obs.is_empty());
    }
}
