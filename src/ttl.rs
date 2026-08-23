//! Tracks cache time-to-live state.

use crate::cachedir::{CacheDir, Clock};

const TTL_SECONDS: u64 = 3_600;

#[derive(Clone, Copy, Debug)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TtlColor {
    Green,
    Yellow,
    Red,
    BrightRed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TtlDisplay {
    Countdown {
        remaining_seconds: u64,
        color: TtlColor,
    },
    Expired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheTtlStatus {
    pub hit_rate: Option<u8>,
    pub ttl: Option<TtlDisplay>,
}

pub struct CacheTtl<C> {
    cache_dir: CacheDir,
    cache_name: String,
    clock: C,
}

impl<C> CacheTtl<C>
where
    C: Clock,
{
    pub fn new(cache_dir: CacheDir, session_id: &str, clock: C) -> Self {
        Self {
            cache_dir,
            cache_name: cache_entry_name(session_id),
            clock,
        }
    }

    pub fn update(&self, usage: TokenUsage) -> CacheTtlStatus {
        let state = self.read_state();
        let now = self.clock.now_epoch();
        let signature = usage.signature();
        let hit_rate = usage.hit_rate();
        let state_matches_usage = match state.as_ref() {
            Some(state) => {
                state.signature.as_deref() == Some(signature.as_str()) && state.started_at.is_some()
            }
            None => false,
        };

        let (started_at, display_hit_rate) = match hit_rate {
            Some(hit_rate) if !state_matches_usage => {
                self.write_state(&signature, now, hit_rate);
                (Some(now), Some(hit_rate))
            }
            Some(hit_rate) => (state.and_then(|state| state.started_at), Some(hit_rate)),
            None => match state {
                Some(state) => (state.started_at, state.last_hit_rate),
                None => (None, None),
            },
        };

        CacheTtlStatus {
            hit_rate: display_hit_rate,
            ttl: started_at.map(|started_at| ttl_display(now, started_at)),
        }
    }

    fn read_state(&self) -> Option<StoredState> {
        let content = self.cache_dir.read(&self.cache_name)?;
        decode_state(&content)
    }

    fn write_state(&self, signature: &str, started_at: u64, hit_rate: u8) {
        let content = format!(
            "{{\"signature\":\"{signature}\",\"started_at\":{started_at},\"last_hit_rate\":\"{hit_rate}\"}}"
        );
        let _write_succeeded = self
            .cache_dir
            .atomic_write(&self.cache_name, content.as_bytes());
    }
}

#[derive(Debug)]
struct StoredState {
    signature: Option<String>,
    started_at: Option<u64>,
    last_hit_rate: Option<u8>,
}

impl TokenUsage {
    fn signature(self) -> String {
        format!(
            "{}:{}:{}",
            self.input_tokens, self.cache_creation_tokens, self.cache_read_tokens
        )
    }

    fn hit_rate(self) -> Option<u8> {
        let total = u128::from(self.input_tokens)
            + u128::from(self.cache_creation_tokens)
            + u128::from(self.cache_read_tokens);
        if total == 0 {
            return None;
        }

        let rounded_rate = (u128::from(self.cache_read_tokens) * 100 + total / 2) / total;
        Some(rounded_rate as u8)
    }
}

fn ttl_display(now: u64, started_at: u64) -> TtlDisplay {
    let elapsed = now.saturating_sub(started_at);
    if elapsed >= TTL_SECONDS {
        return TtlDisplay::Expired;
    }

    let remaining_seconds = TTL_SECONDS - elapsed;
    let color = if remaining_seconds <= 300 {
        if now.is_multiple_of(2) {
            TtlColor::Red
        } else {
            TtlColor::BrightRed
        }
    } else if remaining_seconds <= 1_200 {
        TtlColor::Red
    } else if remaining_seconds <= 2_400 {
        TtlColor::Yellow
    } else {
        TtlColor::Green
    };

    TtlDisplay::Countdown {
        remaining_seconds,
        color,
    }
}

fn cache_entry_name(session_id: &str) -> String {
    let session_key: String = session_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .take(32)
        .collect();
    let session_key = if session_key.is_empty() {
        "default"
    } else {
        session_key.as_str()
    };

    format!("cache-ttl-{session_key}.json")
}

fn decode_state(content: &[u8]) -> Option<StoredState> {
    let value: serde_json::Value = serde_json::from_slice(content).ok()?;
    let signature = value
        .get("signature")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned);
    let started_at = value
        .get("started_at")
        .and_then(serde_json::Value::as_u64)
        .filter(|started_at| *started_at > 0);
    let last_hit_rate = value.get("last_hit_rate").and_then(parse_hit_rate);

    Some(StoredState {
        signature,
        started_at,
        last_hit_rate,
    })
}

fn parse_hit_rate(value: &serde_json::Value) -> Option<u8> {
    match value {
        serde_json::Value::String(rate) => rate.parse::<u8>().ok(),
        serde_json::Value::Number(rate) => rate.as_u64().and_then(|rate| u8::try_from(rate).ok()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    use tempfile::tempdir;

    use crate::cachedir::{CacheDir, Clock};

    use super::{CacheTtl, TokenUsage, TtlColor, TtlDisplay};

    struct FixedClock {
        now: u64,
    }

    impl Clock for FixedClock {
        fn now_epoch(&self) -> u64 {
            self.now
        }
    }

    #[derive(Clone)]
    struct SharedClock {
        now: Arc<AtomicU64>,
    }

    impl Clock for SharedClock {
        fn now_epoch(&self) -> u64 {
            self.now.load(Ordering::Relaxed)
        }
    }

    #[test]
    fn creates_state_for_first_usage_with_a_rounded_hit_rate() -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        let ttl = CacheTtl::new(
            CacheDir::from_paths(None, Some(home.path())),
            "session",
            FixedClock { now: 100 },
        );

        let status = ttl.update(TokenUsage {
            input_tokens: 100,
            cache_creation_tokens: 0,
            cache_read_tokens: 100,
        });

        assert_eq!(status.hit_rate, Some(50));
        assert_eq!(
            status.ttl,
            Some(TtlDisplay::Countdown {
                remaining_seconds: 3_600,
                color: TtlColor::Green,
            })
        );

        let cache = CacheDir::from_paths(None, Some(home.path()));
        let content = cache
            .read("cache-ttl-session.json")
            .ok_or("missing cache ttl state")?;
        let state: serde_json::Value = serde_json::from_slice(&content)?;

        assert_eq!(state["signature"], "100:0:100");
        assert_eq!(state["started_at"], 100);
        assert_eq!(state["last_hit_rate"], "50");

        Ok(())
    }

    #[test]
    fn keeps_a_matching_signature_but_resets_when_usage_changes() -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        let now = Arc::new(AtomicU64::new(100));
        let ttl = CacheTtl::new(
            CacheDir::from_paths(None, Some(home.path())),
            "session",
            SharedClock {
                now: Arc::clone(&now),
            },
        );
        let initial_usage = TokenUsage {
            input_tokens: 100,
            cache_creation_tokens: 0,
            cache_read_tokens: 100,
        };

        let first = ttl.update(initial_usage);
        now.store(200, Ordering::Relaxed);
        let unchanged = ttl.update(initial_usage);
        now.store(201, Ordering::Relaxed);
        let changed = ttl.update(TokenUsage {
            input_tokens: 101,
            cache_creation_tokens: 0,
            cache_read_tokens: 100,
        });

        assert_eq!(
            first.ttl,
            Some(TtlDisplay::Countdown {
                remaining_seconds: 3_600,
                color: TtlColor::Green,
            })
        );
        assert_eq!(
            unchanged.ttl,
            Some(TtlDisplay::Countdown {
                remaining_seconds: 3_500,
                color: TtlColor::Green,
            })
        );
        assert_eq!(
            changed.ttl,
            Some(TtlDisplay::Countdown {
                remaining_seconds: 3_600,
                color: TtlColor::Green,
            })
        );

        Ok(())
    }

    #[test]
    fn reuses_a_last_hit_rate_without_usage_and_skips_an_empty_first_state(
    ) -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        let now = Arc::new(AtomicU64::new(100));
        let ttl = CacheTtl::new(
            CacheDir::from_paths(None, Some(home.path())),
            "session",
            SharedClock {
                now: Arc::clone(&now),
            },
        );
        ttl.update(TokenUsage {
            input_tokens: 1,
            cache_creation_tokens: 0,
            cache_read_tokens: 1,
        });
        now.store(200, Ordering::Relaxed);

        let reused = ttl.update(TokenUsage {
            input_tokens: 0,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
        });

        assert_eq!(reused.hit_rate, Some(50));

        let rate_only_home = tempdir()?;
        let rate_only_cache = CacheDir::from_paths(None, Some(rate_only_home.path()));
        assert!(
            rate_only_cache.atomic_write("cache-ttl-session.json", br#"{"last_hit_rate":"49"}"#,)
        );
        let rate_only = CacheTtl::new(
            CacheDir::from_paths(None, Some(rate_only_home.path())),
            "session",
            FixedClock { now: 200 },
        )
        .update(TokenUsage {
            input_tokens: 0,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
        });

        assert_eq!(rate_only.hit_rate, Some(49));
        assert_eq!(rate_only.ttl, None);

        let fresh_home = tempdir()?;
        let fresh_ttl = CacheTtl::new(
            CacheDir::from_paths(None, Some(fresh_home.path())),
            "session",
            FixedClock { now: 100 },
        );
        let empty = fresh_ttl.update(TokenUsage {
            input_tokens: 0,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
        });

        assert_eq!(empty.hit_rate, None);
        assert_eq!(empty.ttl, None);
        assert!(CacheDir::from_paths(None, Some(fresh_home.path()))
            .read("cache-ttl-session.json")
            .is_none());

        Ok(())
    }

    #[test]
    fn applies_shell_ttl_color_boundaries_and_flash_parity() -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        CacheTtl::new(
            CacheDir::from_paths(None, Some(home.path())),
            "session",
            FixedClock { now: 100 },
        )
        .update(TokenUsage {
            input_tokens: 1,
            cache_creation_tokens: 0,
            cache_read_tokens: 1,
        });

        let cases = [
            (
                1_300,
                TtlDisplay::Countdown {
                    remaining_seconds: 2_400,
                    color: TtlColor::Yellow,
                },
            ),
            (
                2_500,
                TtlDisplay::Countdown {
                    remaining_seconds: 1_200,
                    color: TtlColor::Red,
                },
            ),
            (
                3_400,
                TtlDisplay::Countdown {
                    remaining_seconds: 300,
                    color: TtlColor::Red,
                },
            ),
            (
                3_401,
                TtlDisplay::Countdown {
                    remaining_seconds: 299,
                    color: TtlColor::BrightRed,
                },
            ),
            (3_700, TtlDisplay::Expired),
        ];

        for (now, expected_ttl) in cases {
            let status = CacheTtl::new(
                CacheDir::from_paths(None, Some(home.path())),
                "session",
                FixedClock { now },
            )
            .update(TokenUsage {
                input_tokens: 0,
                cache_creation_tokens: 0,
                cache_read_tokens: 0,
            });

            assert_eq!(status.ttl, Some(expected_ttl), "now={now}");
        }

        Ok(())
    }
}
