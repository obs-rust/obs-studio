//! Read-only expiry decision for the biased strong-reference count.
pub fn expired(owners: Option<i64>) -> bool {
    owners.is_none_or(|owners| owners < 0)
}
