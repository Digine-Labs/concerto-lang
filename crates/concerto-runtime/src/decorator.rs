use std::time::Duration;

use concerto_common::ir::IrDecorator;

/// Parsed retry configuration from @retry decorator.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub backoff: BackoffStrategy,
}

/// Backoff strategy for retry.
#[derive(Debug, Clone)]
pub enum BackoffStrategy {
    None,
    Linear { base_ms: u64 },
    Exponential { base_ms: u64 },
}

/// Parsed timeout configuration from @timeout decorator.
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    pub seconds: u64,
}

/// Find a decorator by name in a list.
pub fn find_decorator<'a>(decorators: &'a [IrDecorator], name: &str) -> Option<&'a IrDecorator> {
    decorators.iter().find(|d| d.name == name)
}

/// Extract RetryConfig from an @retry decorator.
///
/// The args format from the compiler is an array of single-key objects:
/// `[{"max": 3}, {"backoff": "exponential"}]`
pub fn parse_retry(decorator: &IrDecorator) -> RetryConfig {
    let mut max_attempts = 3u32;
    let mut backoff = BackoffStrategy::Exponential { base_ms: 1000 };

    if let Some(args) = &decorator.args {
        if let Some(arr) = args.as_array() {
            for item in arr {
                if let Some(obj) = item.as_object() {
                    if let Some(m) = obj.get("max").and_then(|v| v.as_u64()) {
                        max_attempts = m as u32;
                    }
                    if let Some(b) = obj.get("backoff").and_then(|v| v.as_str()) {
                        backoff = match b {
                            "linear" => BackoffStrategy::Linear { base_ms: 1000 },
                            "none" => BackoffStrategy::None,
                            _ => BackoffStrategy::Exponential { base_ms: 1000 },
                        };
                    }
                }
            }
        }
    }
    RetryConfig {
        max_attempts,
        backoff,
    }
}

/// Extract TimeoutConfig from an @timeout decorator.
///
/// The args format: `[{"seconds": 30}]`
pub fn parse_timeout(decorator: &IrDecorator) -> TimeoutConfig {
    let mut seconds = 30u64;
    if let Some(args) = &decorator.args {
        if let Some(arr) = args.as_array() {
            for item in arr {
                if let Some(obj) = item.as_object() {
                    if let Some(s) = obj.get("seconds").and_then(|v| v.as_u64()) {
                        seconds = s;
                    }
                }
            }
        }
    }
    TimeoutConfig { seconds }
}

/// Calculate backoff delay for a given attempt number (0-indexed).
pub fn backoff_delay(strategy: &BackoffStrategy, attempt: u32) -> Duration {
    match strategy {
        BackoffStrategy::None => Duration::ZERO,
        BackoffStrategy::Linear { base_ms } => {
            Duration::from_millis(base_ms * (attempt as u64 + 1))
        }
        BackoffStrategy::Exponential { base_ms } => {
            Duration::from_millis(base_ms * 2u64.pow(attempt))
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_retry_defaults() {
        let dec = IrDecorator {
            name: "retry".to_string(),
            args: None,
        };
        let config = parse_retry(&dec);
        assert_eq!(config.max_attempts, 3);
        assert!(matches!(
            config.backoff,
            BackoffStrategy::Exponential { base_ms: 1000 }
        ));
    }

    #[test]
    fn parse_retry_custom() {
        let dec = IrDecorator {
            name: "retry".to_string(),
            args: Some(serde_json::json!([
                {"max": 5},
                {"backoff": "linear"}
            ])),
        };
        let config = parse_retry(&dec);
        assert_eq!(config.max_attempts, 5);
        assert!(matches!(
            config.backoff,
            BackoffStrategy::Linear { base_ms: 1000 }
        ));
    }

    #[test]
    fn parse_retry_none_backoff() {
        let dec = IrDecorator {
            name: "retry".to_string(),
            args: Some(serde_json::json!([
                {"max": 2},
                {"backoff": "none"}
            ])),
        };
        let config = parse_retry(&dec);
        assert_eq!(config.max_attempts, 2);
        assert!(matches!(config.backoff, BackoffStrategy::None));
    }

    #[test]
    fn parse_timeout_custom() {
        let dec = IrDecorator {
            name: "timeout".to_string(),
            args: Some(serde_json::json!([{"seconds": 60}])),
        };
        let config = parse_timeout(&dec);
        assert_eq!(config.seconds, 60);
    }

    #[test]
    fn parse_timeout_defaults() {
        let dec = IrDecorator {
            name: "timeout".to_string(),
            args: None,
        };
        let config = parse_timeout(&dec);
        assert_eq!(config.seconds, 30);
    }

    #[test]
    fn backoff_exponential_delays() {
        let strategy = BackoffStrategy::Exponential { base_ms: 1000 };
        assert_eq!(backoff_delay(&strategy, 0), Duration::from_millis(1000));
        assert_eq!(backoff_delay(&strategy, 1), Duration::from_millis(2000));
        assert_eq!(backoff_delay(&strategy, 2), Duration::from_millis(4000));
    }

    #[test]
    fn backoff_linear_delays() {
        let strategy = BackoffStrategy::Linear { base_ms: 500 };
        assert_eq!(backoff_delay(&strategy, 0), Duration::from_millis(500));
        assert_eq!(backoff_delay(&strategy, 1), Duration::from_millis(1000));
        assert_eq!(backoff_delay(&strategy, 2), Duration::from_millis(1500));
    }

    #[test]
    fn backoff_none_is_zero() {
        let strategy = BackoffStrategy::None;
        assert_eq!(backoff_delay(&strategy, 0), Duration::ZERO);
        assert_eq!(backoff_delay(&strategy, 5), Duration::ZERO);
    }

    #[test]
    fn find_decorator_by_name() {
        let decorators = vec![
            IrDecorator {
                name: "retry".to_string(),
                args: Some(serde_json::json!([{"max": 3}])),
            },
            IrDecorator {
                name: "timeout".to_string(),
                args: Some(serde_json::json!([{"seconds": 10}])),
            },
        ];
        assert!(find_decorator(&decorators, "retry").is_some());
        assert!(find_decorator(&decorators, "timeout").is_some());
        assert!(find_decorator(&decorators, "log").is_none());
    }
}
