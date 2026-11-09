//! Exact signed reconnect settings stores; reconnect policy is separate.

/// Store both settings, or report an absent output without touching any fields.
pub fn set(
    fields: Option<(&mut i32, &mut i32)>,
    retry_count: i32,
    retry_sec: i32,
    null_log: impl FnOnce(),
) {
    let _ = (fields, retry_count, retry_sec, null_log);
    todo!("port obs_output_set_reconnect_settings")
}
