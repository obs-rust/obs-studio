//! Borrowed scale-effect identity and loaded mask, without fallback or mutation.
/// Each slot corresponds to the existing scale-effect selector, in enum order.
pub fn loaded<T>(effects: &[Option<T>; 5]) -> u32 {
    effects.iter().enumerate().fold(0, |mask, (i, effect)| {
        mask | if effect.is_some() { 1 << i } else { 0 }
    })
}

/// Unknown selectors choose the default; a missing selected effect stays absent.
pub fn selected<T: Copy>(effects: &[Option<T>; 5], selector: i32) -> Option<T> {
    let index = match selector {
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        _ => 0,
    };
    effects[index]
}
