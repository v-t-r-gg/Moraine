//! Bounded exponential backoff for service readiness (injectable for tests).

use std::sync::Arc;

use crate::suite::http_get_loopback;

/// Default max wait (overridable via `MORAINE_SERVICE_READY_MS` for tests).
pub fn default_service_ready_timeout_ms() -> u64 {
    std::env::var("MORAINE_SERVICE_READY_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10_000)
}

#[derive(Debug, Clone)]
pub struct ServiceReadyResult {
    pub ready: bool,
    pub attempts: u32,
    pub waited_ms: u64,
    pub version: Option<String>,
    pub message: String,
}

/// Probe used by product verification / start wait.
pub trait ServiceProbe: Send + Sync {
    fn wait_ready(&self, max_wait_ms: u64) -> ServiceReadyResult;
}

/// Real product probe: loopback HTTP status with exponential backoff.
pub struct LoopbackServiceProbe;

impl ServiceProbe for LoopbackServiceProbe {
    fn wait_ready(&self, max_wait_ms: u64) -> ServiceReadyResult {
        wait_for_service_ready(max_wait_ms)
    }
}

/// Test double: always ready immediately.
pub struct AlwaysReadyProbe {
    pub version: Option<String>,
}

impl ServiceProbe for AlwaysReadyProbe {
    fn wait_ready(&self, _max_wait_ms: u64) -> ServiceReadyResult {
        ServiceReadyResult {
            ready: true,
            attempts: 1,
            waited_ms: 0,
            version: self.version.clone(),
            message: "background capture is ready (test probe)".into(),
        }
    }
}

/// Test double: always offline.
pub struct AlwaysOfflineProbe;

impl ServiceProbe for AlwaysOfflineProbe {
    fn wait_ready(&self, max_wait_ms: u64) -> ServiceReadyResult {
        ServiceReadyResult {
            ready: false,
            attempts: 1,
            waited_ms: max_wait_ms,
            version: None,
            message: format!("background capture not ready after {max_wait_ms}ms (test probe)"),
        }
    }
}

pub fn default_service_probe() -> Arc<dyn ServiceProbe> {
    Arc::new(LoopbackServiceProbe)
}

/// Poll loopback status until healthy or timeout.
///
/// Schedule: 100, 200, 400, 800, 1000… ms up to `max_wait_ms` (default 10s).
pub fn wait_for_service_ready(max_wait_ms: u64) -> ServiceReadyResult {
    let mut waited = 0u64;
    let mut delay = 100u64;
    let mut attempts = 0u32;
    loop {
        attempts += 1;
        match http_get_loopback(33111, "/status") {
            Ok(body) => {
                let v: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
                let online = v
                    .get("online")
                    .and_then(|x| x.as_bool())
                    .or_else(|| v.get("status").and_then(|s| s.as_str()).map(|s| s == "ok"))
                    .unwrap_or(true);
                let version = v
                    .get("version")
                    .or_else(|| v.get("productVersion"))
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string());
                if online {
                    return ServiceReadyResult {
                        ready: true,
                        attempts,
                        waited_ms: waited,
                        version,
                        message: "background capture is ready".into(),
                    };
                }
            }
            Err(_) => {}
        }
        if waited >= max_wait_ms {
            return ServiceReadyResult {
                ready: false,
                attempts,
                waited_ms: waited,
                version: None,
                message: format!("background capture not ready after {waited}ms"),
            };
        }
        let sleep_ms = delay.min(max_wait_ms.saturating_sub(waited)).max(1);
        std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
        waited += sleep_ms;
        delay = (delay.saturating_mul(2)).min(1000);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_fails_closed_quickly_when_offline() {
        let r = wait_for_service_ready(300);
        assert!(r.attempts >= 1);
        assert!(r.waited_ms <= 500 || r.ready);
    }

    #[test]
    fn always_ready_probe_is_immediate() {
        let p = AlwaysReadyProbe {
            version: Some("0.1.0".into()),
        };
        let r = p.wait_ready(10_000);
        assert!(r.ready);
        assert_eq!(r.waited_ms, 0);
    }
}
