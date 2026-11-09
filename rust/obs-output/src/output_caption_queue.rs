//! Queue copied CEA-708 triplets on each configured output track.

/// A track borrowed while its caption mutex is held.
pub trait Track {
    fn append(&mut self, packet: [u8; 3]);
}

/// Configured tracks; missing slots are skipped without inspecting captions.
pub trait Tracks {
    type Locked<'a>: Track
    where
        Self: 'a;
    fn slots(&self) -> usize;
    fn lock(&mut self, index: usize) -> Option<Self::Locked<'_>>;
}

/// Packet access is deferred until a configured track has been locked.
pub trait Packets {
    fn count(&self) -> usize;
    fn packet(&self, index: usize) -> [u8; 3];
}

impl Packets for [[u8; 3]] {
    fn count(&self) -> usize {
        self.len()
    }
    fn packet(&self, index: usize) -> [u8; 3] {
        self[index]
    }
}

/// Append every packet in order, preserving the track's existing queue.
pub fn enqueue(tracks: &mut impl Tracks, packets: &(impl Packets + ?Sized)) {
    let _ = (tracks, packets);
    todo!()
}
