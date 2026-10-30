//! Proptest case counts for the Tier 3 parity tests, scaled by CI level (#2593).
//!
//! `ProptestConfig::with_cases(N)` ignores `PROPTEST_CASES`, so every parity
//! module writes `with_cases(obs_c_oracle::parity::cases(N))` instead. The
//! level comes from `OBS_PARITY_CASES`:
//!
//! - unset or `full`: `N` (local runs, nightly, release gate);
//! - `reduced`: [`REDUCED_PERCENT`] of `N` (PR, merge queue, master push);
//! - an integer `1..=100`: that percent of `N`.
//!
//! A scaled count never drops below [`FLOOR`] (or `N` itself when smaller).
//! Any other value panics, so a typo never silently cuts coverage.

/// Environment variable that selects the case level.
pub const ENV: &str = "OBS_PARITY_CASES";
/// Percent of the written count that `reduced` keeps.
pub const REDUCED_PERCENT: u32 = 10;
/// Fewest cases a scaled module runs.
pub const FLOOR: u32 = 64;

/// The case count for a module written with `n` cases, at the level in [`ENV`].
pub fn cases(n: u32) -> u32 {
    scale(n, std::env::var(ENV).ok().as_deref())
}

/// The case count for `n` at `level` (the value of [`ENV`], `None` when unset).
pub fn scale(n: u32, level: Option<&str>) -> u32 {
    let percent = match level.map(str::trim) {
        None | Some("" | "full") => return n,
        Some("reduced") => REDUCED_PERCENT,
        Some(other) => match other.parse::<u32>() {
            Ok(p @ 1..=100) => p,
            _ => panic!("{ENV}={other:?}: expected `full`, `reduced` or a percent 1..=100"),
        },
    };
    let scaled = u32::try_from(u64::from(n) * u64::from(percent) / 100).unwrap_or(n);
    scaled.max(FLOOR.min(n))
}
