//! Copy the stored crop, preserving null argument precedence.

/// The original getter checks the item before the destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Missing { Item, Crop }

/// Copy all four signed crop integers, or leave the destination untouched.
pub fn get(value: Option<[i32; 4]>, output: Option<&mut [i32; 4]>) -> Result<(), Missing> {
    let _ = (value, output);
    todo!("#3897 crop getter")
}
