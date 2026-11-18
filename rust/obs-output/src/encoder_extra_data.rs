//! Optional encoder extra-data dispatch, independent of private C layouts.

/// Invoke the active callback once only when its context data exists.
/// A missing encoder emits its diagnostic; outputs belong entirely to the callback.
pub fn get_extra_data<F: FnOnce() -> bool>(
    encoder: Option<(bool, Option<F>)>,
    null_encoder: impl FnOnce(),
) -> bool {
    let Some((has_data, callback)) = encoder else {
        null_encoder();
        return false;
    };
    has_data && callback.is_some_and(|callback| callback())
}
