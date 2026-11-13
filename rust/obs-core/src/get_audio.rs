//! Borrow the current core audio output without changing its ownership.
pub fn get_audio<A>(audio: A) -> A {
    let _ = audio;
    todo!("port obs_get_audio")
}
