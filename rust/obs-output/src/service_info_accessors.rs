//! A service's own copy of its plugin table and the plugin data `create()`
//! returned (`libobs/obs-service-info-accessors.c.h`, #3840). The fields
//! stay in C; these decide what is written and what is handed back.

/// Copies the whole table into the service, by value: a later change to
/// the source does not reach the service.
pub fn set_info<I>(table: &mut I, info: I) {
    *table = info;
}

/// The service's own table, borrowed: never the source it was copied from.
pub fn info_table<I>(table: &I) -> &I {
    table
}

/// Stores the plugin data as it is, NULL included.
pub fn set_plugin_data<D>(slot: &mut D, data: D) {
    *slot = data;
}

/// The stored plugin data, read without changing the service.
pub fn plugin_data<D: Copy>(slot: &D) -> D {
    *slot
}
