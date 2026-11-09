//! Borrowed scale-effect identity and loaded mask, without fallback or mutation.
/// Each slot corresponds to the existing scale-effect selector, in enum order.
pub fn loaded<T>(effects: &[Option<T>; 5]) -> u32 {
    let _ = effects;
    todo!()
}

/// Unknown selectors choose the default; a missing selected effect stays absent.
pub fn selected<T: Copy>(effects: &[Option<T>; 5], selector: i32) -> Option<T> {
    let _ = (effects, selector);
    todo!()
}
