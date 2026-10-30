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
    let percent = percent_at(level);
    if percent == 100 {
        return n;
    }
    let scaled = u32::try_from(u64::from(n) * u64::from(percent) / 100).unwrap_or(n);
    scaled.max(FLOOR.min(n))
}

/// The percent of an exhaustive sweep or fixed grid that runs at `level`
/// (100 at `full`). Same levels as [`scale`] (#2888).
pub fn percent_at(level: Option<&str>) -> u32 {
    match level.map(str::trim) {
        None | Some("" | "full") => 100,
        Some("reduced") => REDUCED_PERCENT,
        Some(other) => match other.parse::<u32>() {
            Ok(p @ 1..=100) => p,
            _ => panic!("{ENV}={other:?}: expected `full`, `reduced` or a percent 1..=100"),
        },
    }
}

/// A fixed 64-bit mix (splitmix64's finalizer): spreads grid indices and
/// sweep windows so a reduced level favours no bit pattern.
fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// Whether item `i` of a large fixed grid runs at the level in [`ENV`]:
/// every item at `full`, otherwise a fixed, spread-out [`percent_at`] share.
pub fn keep(i: u64) -> bool {
    // Grids call this once per item: read the level once per process.
    static PERCENT: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    let percent = *PERCENT.get_or_init(|| percent_at(std::env::var(ENV).ok().as_deref()));
    keep_percent(i, percent)
}

/// [`keep`] at `level`.
pub fn keep_at(i: u64, level: Option<&str>) -> bool {
    keep_percent(i, percent_at(level))
}

fn keep_percent(i: u64, percent: u32) -> bool {
    percent == 100 || mix(i) % 100 < u64::from(percent)
}

/// The values of `range` an exhaustive sweep visits at the level in [`ENV`]:
/// all of them at `full`, otherwise one value in each window of
/// `100 / percent` values, at a hashed offset so no bit pattern is favoured.
pub fn sweep(range: std::ops::Range<u64>) -> impl Iterator<Item = u64> + use<> {
    sweep_at(range, std::env::var(ENV).ok().as_deref())
}

/// [`sweep`] at `level`.
pub fn sweep_at(
    range: std::ops::Range<u64>,
    level: Option<&str>,
) -> impl Iterator<Item = u64> + use<> {
    let window = u64::from(100 / percent_at(level));
    let (start, end) = (range.start, range.end.max(range.start));
    let windows = (end - start).div_ceil(window);
    (0..windows).map(move |w| {
        let lo = start + w * window;
        let len = window.min(end - lo);
        if window == 1 { lo } else { lo + mix(w) % len }
    })
}
