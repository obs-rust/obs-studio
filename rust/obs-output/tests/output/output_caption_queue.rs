//! Tier 1 mirrors test_output_caption_append's direct queue assertions.
use obs_output::output_caption_queue::{Track, Tracks, enqueue};

struct Queues(Vec<Option<Vec<u8>>>);
struct LockedVec<'a>(&'a mut Vec<u8>);
impl Track for LockedVec<'_> {
    fn append(&mut self, packet: [u8; 3]) {
        self.0.extend_from_slice(&packet);
    }
}
impl Tracks for Queues {
    type Locked<'a> = LockedVec<'a>;
    fn slots(&self) -> usize {
        self.0.len()
    }
    fn lock(&mut self, index: usize) -> Option<Self::Locked<'_>> {
        self.0[index].as_mut().map(LockedVec)
    }
}

#[test]
fn test_output_caption_append_sparse_tracks_copy_and_append() {
    for mask in [0, 1, 32, 21, 63] {
        let mut tracks = Queues(
            (0..6)
                .map(|i| (mask & (1 << i) != 0).then(|| vec![9, 8, 7]))
                .collect(),
        );
        enqueue(&mut tracks, &[][..]);
        let mut packets = [[4, 193, 194], [4, 67, 196]];
        enqueue(&mut tracks, &packets[..]);
        packets.fill([255; 3]);
        enqueue(&mut tracks, &[[5, 6, 7]][..]);
        enqueue(&mut tracks, &[][..]);
        for (i, queue) in tracks.0.iter().enumerate() {
            if mask & (1 << i) == 0 {
                assert!(queue.is_none());
            } else {
                assert_eq!(
                    queue.as_ref().unwrap(),
                    &[9, 8, 7, 4, 193, 194, 4, 67, 196, 5, 6, 7]
                );
            }
        }
    }
}
