//! Borrow the current core audio output without changing its ownership.
pub fn get_audio<A>(audio: A) -> A {
    audio
}
