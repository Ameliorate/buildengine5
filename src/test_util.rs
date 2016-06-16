//! This module provides various utilities and abstractions to aid in unit testing.
//!
//! While these structs and functions are made avalable in release mode, it is not encouraged to use them.
//! You should instead request the functionality to be better exposed in official APIs.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use env_logger;

/// Ammount of time to sleep when syncronizing threading.
///
/// Increase if tests fail when the should succeed too much.
pub const TEST_SLEEP_TIME_MILLIS: u64 = 500;

/// Provides an abstraction for testing weather or not a code path was taken, and how many times.
///
/// All state for the Tattle is done using atomics, so locks are not needed for thread-safety.
#[derive(Debug, Clone)]
pub struct Tattle(Arc<AtomicUsize>);

impl Tattle {
    /// Constructs a new Tattle.
    pub fn new() -> Tattle {
        Tattle(Arc::from(AtomicUsize::new(0)))
    }

    /// Increment the internal value of the Tattle.
    ///
    /// This is done atomically, so locks are not needed.
    pub fn call(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }

    /// Gets the internal value of the Tattle.
    ///
    /// This is done atomically, so locks are not needed.
    pub fn get(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }

    /// Compares the value of the Tattle before and after running the given closure.
    ///
    /// If the value changed, true is returned, else false.
    pub fn changed<F>(&self, closure: F) -> bool
        where F: FnOnce()
    {
        let old = self.get();
        closure();
        let new = self.get();
        old != new
    }

    /// Compares the value of the Tattle before and after running the given closure.
    ///
    /// If the value changed, the a panic is raised. The function returns otherwise.
    pub fn assert_changed<F>(&self, closure: F)
        where F: FnOnce()
    {
        let old = self.get();
        closure();
        let new = self.get();
        assert!(new != old);
    }
}

/// Start the logging facilities, ignoring any errors about having already initalized it.
pub fn start_log_once() {
    let _ = env_logger::init();
}
