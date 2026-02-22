//! Test framework for Astra
//!
//! Provides deterministic test execution with JSON output.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Test result status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
}

/// A single test result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// Test name
    pub name: String,
    /// Module containing the test
    pub module: String,
    /// Test status
    pub status: TestStatus,
    /// Duration of the test
    #[serde(with = "duration_millis")]
    pub duration: Duration,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Stack trace if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack_trace: Option<Vec<StackFrame>>,
}

/// A stack frame in a failure trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackFrame {
    /// Function name
    pub function: String,
    /// File path
    pub file: String,
    /// Line number
    pub line: u32,
    /// Column number
    pub column: u32,
}

/// Summary of a test run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSummary {
    /// Total number of tests
    pub total: usize,
    /// Number of passed tests
    pub passed: usize,
    /// Number of failed tests
    pub failed: usize,
    /// Number of skipped tests
    pub skipped: usize,
    /// Total duration
    #[serde(with = "duration_millis")]
    pub duration: Duration,
    /// Random seed used
    pub seed: u64,
}

/// Complete test run results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRunResults {
    /// Summary statistics
    pub summary: TestSummary,
    /// Individual test results
    pub tests: Vec<TestResult>,
}

/// Configuration for test runs
#[derive(Debug, Clone)]
pub struct TestConfig {
    /// Random seed for determinism
    pub seed: u64,
    /// Filter pattern for test names
    pub filter: Option<String>,
    /// Whether to stop on first failure
    pub fail_fast: bool,
    /// Maximum number of property test iterations
    pub property_iterations: usize,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            seed: 0,
            filter: None,
            fail_fast: false,
            property_iterations: 100,
        }
    }
}

/// Test runner
pub struct TestRunner {
    config: TestConfig,
    results: Vec<TestResult>,
}

impl TestRunner {
    /// Create a new test runner with default configuration
    pub fn new() -> Self {
        Self::with_config(TestConfig::default())
    }

    /// Create a test runner with specific configuration
    pub fn with_config(config: TestConfig) -> Self {
        Self {
            config,
            results: Vec::new(),
        }
    }

    /// Get the configuration
    pub fn config(&self) -> &TestConfig {
        &self.config
    }

    /// Record a test result
    pub fn record(&mut self, result: TestResult) {
        self.results.push(result);
    }

    /// Check if we should stop (fail_fast mode)
    pub fn should_stop(&self) -> bool {
        self.config.fail_fast && self.results.iter().any(|r| r.status == TestStatus::Failed)
    }

    /// Get current results
    pub fn results(&self) -> &[TestResult] {
        &self.results
    }

    /// Generate final results
    pub fn finish(self, total_duration: Duration) -> TestRunResults {
        let passed = self
            .results
            .iter()
            .filter(|r| r.status == TestStatus::Passed)
            .count();
        let failed = self
            .results
            .iter()
            .filter(|r| r.status == TestStatus::Failed)
            .count();
        let skipped = self
            .results
            .iter()
            .filter(|r| r.status == TestStatus::Skipped)
            .count();

        TestRunResults {
            summary: TestSummary {
                total: self.results.len(),
                passed,
                failed,
                skipped,
                duration: total_duration,
                seed: self.config.seed,
            },
            tests: self.results,
        }
    }
}

impl Default for TestRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Assertion helpers for tests
pub mod assert {
    /// Assert that two values are equal
    pub fn assert_eq<T: PartialEq + std::fmt::Debug>(left: T, right: T) -> Result<(), String> {
        if left == right {
            Ok(())
        } else {
            Err(format!(
                "assertion failed: `left == right`\n  left: {:?}\n right: {:?}",
                left, right
            ))
        }
    }

    /// Assert that two values are not equal
    pub fn assert_ne<T: PartialEq + std::fmt::Debug>(left: T, right: T) -> Result<(), String> {
        if left != right {
            Ok(())
        } else {
            Err(format!(
                "assertion failed: `left != right`\n  left: {:?}\n right: {:?}",
                left, right
            ))
        }
    }

    /// Assert that a condition is true
    pub fn assert_true(condition: bool) -> Result<(), String> {
        if condition {
            Ok(())
        } else {
            Err("assertion failed: expected true".to_string())
        }
    }

    /// Assert that a condition is false
    pub fn assert_false(condition: bool) -> Result<(), String> {
        if !condition {
            Ok(())
        } else {
            Err("assertion failed: expected false".to_string())
        }
    }
}

/// Property testing support
pub mod property {
    /// Generator for random values
    pub trait Generator<T> {
        fn generate(&self, rng: &mut dyn FnMut() -> u64) -> T;
        fn shrink(&self, value: T) -> Vec<T>;
    }

    /// Integer generator
    pub struct IntGenerator {
        pub min: i64,
        pub max: i64,
    }

    impl Generator<i64> for IntGenerator {
        fn generate(&self, rng: &mut dyn FnMut() -> u64) -> i64 {
            let range = (self.max - self.min + 1) as u64;
            self.min + (rng() % range) as i64
        }

        fn shrink(&self, value: i64) -> Vec<i64> {
            let mut shrinks = Vec::new();
            if value > 0 {
                shrinks.push(value / 2);
                shrinks.push(value - 1);
            } else if value < 0 {
                shrinks.push(value / 2);
                shrinks.push(value + 1);
            }
            if value != 0 {
                shrinks.push(0);
            }
            shrinks
        }
    }

    /// Boolean generator
    pub struct BoolGenerator;

    impl Generator<bool> for BoolGenerator {
        fn generate(&self, rng: &mut dyn FnMut() -> u64) -> bool {
            rng().is_multiple_of(2)
        }

        fn shrink(&self, value: bool) -> Vec<bool> {
            if value {
                vec![false]
            } else {
                vec![]
            }
        }
    }

    /// Run a property test
    pub fn run_property<T, F>(
        generator: &dyn Generator<T>,
        iterations: usize,
        seed: u64,
        property: F,
    ) -> Result<(), (T, String)>
    where
        T: Clone,
        F: Fn(&T) -> Result<(), String>,
    {
        let mut rng_state = seed;
        let mut rng = || {
            rng_state ^= rng_state << 13;
            rng_state ^= rng_state >> 7;
            rng_state ^= rng_state << 17;
            rng_state
        };

        for _ in 0..iterations {
            let value = generator.generate(&mut rng);

            if let Err(msg) = property(&value) {
                // Try to shrink the failing case
                let mut smallest = value.clone();
                let mut smallest_msg = msg;

                let mut to_try = generator.shrink(value);
                while let Some(candidate) = to_try.pop() {
                    if let Err(msg) = property(&candidate) {
                        smallest = candidate.clone();
                        smallest_msg = msg;
                        to_try.extend(generator.shrink(candidate));
                    }
                }

                return Err((smallest, smallest_msg));
            }
        }

        Ok(())
    }
}

// Serde helper for Duration in milliseconds
mod duration_millis {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_millis() as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}
#[cfg(test)]
mod tests;
