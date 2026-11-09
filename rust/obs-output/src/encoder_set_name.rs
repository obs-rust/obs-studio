//! Encoder names are not uniquified; ignore absent, empty and byte-equal names.
pub fn should_rename(current: &[u8], name: Option<&[u8]>) -> bool {
    let _ = (current, name);
    todo!()
}
