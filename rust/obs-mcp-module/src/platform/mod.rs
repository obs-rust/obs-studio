std::cfg_select! {
 unix => {mod unix; pub use unix::*;}
 _ => {pub fn version()->u32{0} pub fn load()->bool{false} pub fn unload(){} }
}
