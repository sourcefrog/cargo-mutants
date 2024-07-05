// Copyright 2021-2024 Martin Pool

//! Calculation of timeouts for the build and test phases.

use std::{cmp::max, time::Duration};

use tracing::{info, warn};

use crate::{
    options::Options,
    outcome::{Phase, ScenarioOutcome},
};

#[derive(Debug, Copy, Clone)]
pub struct Timeouts {
    pub build: Duration,
    pub test: Duration,
}

impl Timeouts {
    pub fn for_baseline(options: &Options) -> Timeouts {
        Timeouts {
            test: options.test_timeout.unwrap_or(Duration::MAX),
            build: options.build_timeout.unwrap_or(Duration::MAX),
        }
    }

    pub fn from_baseline(baseline: &ScenarioOutcome, options: &Options) -> Timeouts {
        Timeouts {
            build: build_timeout(
                baseline.phase_result(Phase::Build).map(|pr| pr.duration),
                options,
            ),
            test: test_timeout(
                baseline.phase_result(Phase::Test).map(|pr| pr.duration),
                options,
            ),
        }
    }

    pub fn without_baseline(options: &Options) -> Timeouts {
        Timeouts {
            build: build_timeout(None, options),
            test: test_timeout(None, options),
        }
    }
}

const FALLBACK_TIMEOUT_SECS: u64 = 300;
fn warn_fallback_timeout(phase_name: &str, option: &str) {
    warn!("An explicit {phase_name} timeout is recommended when using {option}; using {FALLBACK_TIMEOUT_SECS} seconds by default");
}

fn phase_timeout(
    phase: Phase,
    explicit_timeout: Option<Duration>,
    baseline_duration: Option<Duration>,
    minimum: Duration,
    multiplier: f64,
    options: &Options,
) -> Duration {
    if let Some(timeout) = explicit_timeout {
        return timeout;
    }

    match baseline_duration {
        Some(_) if options.in_place && phase != Phase::Test => {
            warn_fallback_timeout(phase.name(), "--in-place");
            Duration::from_secs(FALLBACK_TIMEOUT_SECS)
        }
        Some(baseline_duration) => {
            let timeout = max(
                minimum,
                Duration::from_secs((baseline_duration.as_secs_f64() * multiplier).ceil() as u64),
            );

            if options.show_times {
                info!(
                    "Auto-set {} timeout to {}",
                    phase.name(),
                    humantime::format_duration(timeout)
                );
            }
            timeout
        }
        None => {
            warn_fallback_timeout(phase.name(), "--baseline=skip");
            Duration::from_secs(FALLBACK_TIMEOUT_SECS)
        }
    }
}

fn test_timeout(baseline_duration: Option<Duration>, options: &Options) -> Duration {
    phase_timeout(
        Phase::Test,
        options.test_timeout,
        baseline_duration,
        options.minimum_test_timeout,
        options.test_timeout_multiplier.unwrap_or(5.0),
        options,
    )
}

fn build_timeout(baseline_duration: Option<Duration>, options: &Options) -> Duration {
    phase_timeout(
        Phase::Build,
        options.build_timeout,
        baseline_duration,
        Duration::from_secs(20),
        options.build_timeout_multiplier.unwrap_or(2.0),
        options,
    )
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use clap::Parser;
    use indoc::indoc;

    use super::*;
    use crate::{config::Config, Args};

    #[test]
    fn timeout_multiplier_from_option() {
        let args = Args::parse_from(["mutants", "--timeout-multiplier", "1.5"]);
        let options = Options::new(&args, &Config::default()).unwrap();

        assert_eq!(options.test_timeout_multiplier, Some(1.5));
        assert_eq!(
            test_timeout(Some(Duration::from_secs(40)), &options),
            Duration::from_secs(60),
        );
    }

    #[test]
    fn test_timeout_unaffected_by_in_place_build() {
        let args = Args::parse_from(["mutants", "--timeout-multiplier", "1.5", "--in-place"]);
        let options = Options::new(&args, &Config::default()).unwrap();

        assert_eq!(
            test_timeout(Some(Duration::from_secs(40)), &options),
            Duration::from_secs(60),
        );
    }

    #[test]
    fn build_timeout_multiplier_from_option() {
        let args = Args::parse_from(["mutants", "--build-timeout-multiplier", "1.5"]);
        let options = Options::new(&args, &Config::default()).unwrap();

        assert_eq!(options.build_timeout_multiplier, Some(1.5));
        assert_eq!(
            build_timeout(Some(Duration::from_secs(40)), &options),
            Duration::from_secs(60),
        );
    }

    #[test]
    fn build_timeout_is_affected_by_in_place_build() {
        let args = Args::parse_from(["mutants", "--build-timeout-multiplier", "1.5", "--in-place"]);
        let options = Options::new(&args, &Config::default()).unwrap();

        assert_eq!(
            build_timeout(Some(Duration::from_secs(40)), &options),
            Duration::from_secs(300),
        );
    }

    #[test]
    fn timeout_multiplier_from_config() {
        let args = Args::parse_from(["mutants"]);
        let config = Config::from_str(indoc! {r#"
            timeout_multiplier = 2.0
        "#})
        .unwrap();
        let options = Options::new(&args, &config).unwrap();

        assert_eq!(options.test_timeout_multiplier, Some(2.0));
        assert_eq!(
            test_timeout(Some(Duration::from_secs(42)), &options),
            Duration::from_secs(42 * 2),
        );
    }

    #[test]
    fn build_timeout_multiplier_from_config() {
        let args = Args::parse_from(["mutants"]);
        let config = Config::from_str(indoc! {r#"
            build_timeout_multiplier = 2.0
        "#})
        .unwrap();
        let options = Options::new(&args, &config).unwrap();

        assert_eq!(options.build_timeout_multiplier, Some(2.0));
        assert_eq!(
            build_timeout(Some(Duration::from_secs(42)), &options),
            Duration::from_secs(42 * 2),
        );
    }

    #[test]
    fn timeout_multiplier_default() {
        let args = Args::parse_from(["mutants"]);
        let options = Options::new(&args, &Config::default()).unwrap();

        assert_eq!(options.test_timeout_multiplier, None);
        assert_eq!(
            test_timeout(Some(Duration::from_secs(42)), &options),
            Duration::from_secs(42 * 5),
        );
    }

    #[test]
    fn build_timeout_multiplier_default() {
        let args = Args::parse_from(["mutants"]);
        let options = Options::new(&args, &Config::default()).unwrap();

        assert_eq!(options.build_timeout_multiplier, None);
        assert_eq!(
            build_timeout(Some(Duration::from_secs(42)), &options),
            Duration::from_secs(42 * 2),
        );
    }

    #[test]
    fn timeout_from_option() {
        let args = Args::parse_from(["mutants", "--timeout=8"]);
        let options = Options::new(&args, &Config::default()).unwrap();

        assert_eq!(options.test_timeout, Some(Duration::from_secs(8)));
    }

    #[test]
    fn build_timeout_from_option() {
        let args = Args::parse_from(["mutants", "--build-timeout=4"]);
        let options = Options::new(&args, &Config::default()).unwrap();

        assert_eq!(options.build_timeout, Some(Duration::from_secs(4)));
    }

    #[test]
    fn timeout_multiplier_default_with_baseline_skip() {
        // The --baseline option is not used to set the timeout but it's
        // indicative of the realistic situation.
        let args = Args::parse_from(["mutants", "--baseline", "skip"]);
        let options = Options::new(&args, &Config::default()).unwrap();

        assert_eq!(options.test_timeout_multiplier, None);
        assert_eq!(test_timeout(None, &options), Duration::from_secs(300),);
    }

    #[test]
    fn build_timeout_multiplier_default_with_baseline_skip() {
        // The --baseline option is not used to set the timeout but it's
        // indicative of the realistic situation.
        let args = Args::parse_from(["mutants", "--baseline", "skip"]);
        let options = Options::new(&args, &Config::default()).unwrap();

        assert_eq!(options.build_timeout_multiplier, None);
        assert_eq!(build_timeout(None, &options), Duration::from_secs(300),);
    }
}
