//! Agentic WASM - Zero-Latency Safety Kernel
//!
//! A WebAssembly implementation of the CircuitBreaker pattern for
//! use in Node.js and browser environments without Python dependencies.

use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

// Thread-local storage for the circuit breaker state
thread_local! {
    static BREAKER: RefCell<CircuitBreakerState> = RefCell::new(CircuitBreakerState::new(5, 60));
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BreakerState {
    Closed,
    Open,
    HalfOpen,
}

impl BreakerState {
    fn as_str(&self) -> &'static str {
        match self {
            BreakerState::Closed => "closed",
            BreakerState::Open => "open",
            BreakerState::HalfOpen => "half_open",
        }
    }
}

struct CircuitBreakerState {
    state: BreakerState,
    failure_count: u32,
    success_count: u32,
    failure_threshold: u32,
    recovery_timeout: u64,
    last_failure_time: Option<u64>,
    half_open_calls: u32,
    half_open_max: u32,
}

impl CircuitBreakerState {
    fn new(failure_threshold: u32, recovery_timeout: u64) -> Self {
        Self {
            state: BreakerState::Closed,
            failure_count: 0,
            success_count: 0,
            failure_threshold,
            recovery_timeout,
            last_failure_time: None,
            half_open_calls: 0,
            half_open_max: 3,
        }
    }
}

/// Initialize the circuit breaker with custom thresholds
#[wasm_bindgen]
pub fn init_breaker(failure_threshold: u32, recovery_timeout: u64) {
    BREAKER.with(|b| {
        let mut breaker = b.borrow_mut();
        breaker.failure_threshold = failure_threshold;
        breaker.recovery_timeout = recovery_timeout;
        breaker.state = BreakerState::Closed;
        breaker.failure_count = 0;
        breaker.success_count = 0;
    });
}

/// Check if a request should be allowed
#[wasm_bindgen]
pub fn allow_request(current_time_ms: u64) -> bool {
    BREAKER.with(|b| {
        let mut breaker = b.borrow_mut();
        
        // Check for recovery from Open state
        if breaker.state == BreakerState::Open {
            if let Some(last_failure) = breaker.last_failure_time {
                let elapsed_secs = (current_time_ms - last_failure) / 1000;
                if elapsed_secs >= breaker.recovery_timeout {
                    breaker.state = BreakerState::HalfOpen;
                    breaker.half_open_calls = 0;
                    breaker.success_count = 0;
                }
            }
        }
        
        match breaker.state {
            BreakerState::Closed => true,
            BreakerState::Open => false,
            BreakerState::HalfOpen => {
                if breaker.half_open_calls < breaker.half_open_max {
                    breaker.half_open_calls += 1;
                    true
                } else {
                    false
                }
            }
        }
    })
}

/// Record a successful operation
#[wasm_bindgen]
pub fn record_success() {
    BREAKER.with(|b| {
        let mut breaker = b.borrow_mut();
        breaker.success_count += 1;
        
        if breaker.state == BreakerState::HalfOpen {
            if breaker.success_count >= breaker.half_open_max {
                breaker.state = BreakerState::Closed;
                breaker.failure_count = 0;
                breaker.success_count = 0;
            }
        }
    });
}

/// Record a failed operation
#[wasm_bindgen]
pub fn record_failure(current_time_ms: u64) {
    BREAKER.with(|b| {
        let mut breaker = b.borrow_mut();
        breaker.failure_count += 1;
        breaker.last_failure_time = Some(current_time_ms);
        
        if breaker.state == BreakerState::HalfOpen {
            breaker.state = BreakerState::Open;
        } else if breaker.failure_count >= breaker.failure_threshold {
            breaker.state = BreakerState::Open;
        }
    });
}

/// Get current breaker state as JSON string
#[wasm_bindgen]
pub fn get_status() -> String {
    BREAKER.with(|b| {
        let breaker = b.borrow();
        format!(
            r#"{{"state":"{}","failures":{},"successes":{}}}"#,
            breaker.state.as_str(),
            breaker.failure_count,
            breaker.success_count
        )
    })
}

/// Force the breaker open (kill switch)
#[wasm_bindgen]
pub fn force_open(current_time_ms: u64) {
    BREAKER.with(|b| {
        let mut breaker = b.borrow_mut();
        breaker.state = BreakerState::Open;
        breaker.last_failure_time = Some(current_time_ms);
    });
}

/// Reset the breaker to closed state
#[wasm_bindgen]
pub fn reset_breaker() {
    BREAKER.with(|b| {
        let mut breaker = b.borrow_mut();
        breaker.state = BreakerState::Closed;
        breaker.failure_count = 0;
        breaker.success_count = 0;
        breaker.half_open_calls = 0;
        breaker.last_failure_time = None;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_breaker_starts_closed() {
        reset_breaker();
        assert!(allow_request(0));
    }

    #[test]
    fn test_breaker_opens_after_failures() {
        init_breaker(3, 60);
        reset_breaker();
        
        record_failure(1000);
        record_failure(2000);
        assert!(allow_request(3000)); // Still closed after 2 failures
        
        record_failure(3000);
        assert!(!allow_request(4000)); // Now open after 3 failures
    }
}
