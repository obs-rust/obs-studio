//! Tier 1 for `obs-encoder-packet-ownership.c.h`: the safe core's step
//! order against a recording host, and every assertion of
//! `test_encoder_packet_ownership.c` through the exports.
#![allow(unsafe_code)]
use obs_encoder::ffi::packet_ownership::{
    encoder_packet, obs_encoder_packet_create_instance, obs_encoder_packet_ref,
    obs_encoder_packet_release,
};
use obs_encoder::packet_ownership::{PacketHost, create_instance, reference, release};
use obs_util::ffi::bmem::bnum_allocs;
use obs_util::platform::CLong;
use std::ptr;

// ---------------------------------------------------------------------------
// the safe core, over a recording host

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Assign(u8, u8),
    Allocate(usize),
    Attach(u8, u16),
    InitCount(u16),
    CopyPayload(u8, u8),
    Increment(u16),
    Decrement(u16),
    Free(u16),
    Zero(u8),
}

/// Packets are names (`u8`); a packet's block, when it has one, comes
/// from `blocks`. `decrement` reports zero for the counts in `last`.
#[derive(Default)]
struct Recorder {
    steps: Vec<Step>,
    sizes: Vec<(u8, usize)>,
    blocks: Vec<(u8, u16)>,
    next_block: u16,
    last: Vec<u16>,
}

impl PacketHost for Recorder {
    type Packet = u8;
    type Block = u16;

    fn header_size(&self) -> usize {
        8
    }
    fn size(&self, packet: u8) -> usize {
        // `&self`: recorded through a cell would be noise; the callers'
        // order shows in the mutating steps around it.
        self.sizes
            .iter()
            .find(|(p, _)| *p == packet)
            .map_or(0, |(_, s)| *s)
    }
    fn block(&self, packet: u8) -> Option<u16> {
        self.blocks
            .iter()
            .find(|(p, _)| *p == packet)
            .map(|(_, b)| *b)
    }
    fn assign(&mut self, dst: u8, src: u8) {
        self.steps.push(Step::Assign(dst, src));
    }
    fn allocate(&mut self, bytes: usize) -> u16 {
        self.steps.push(Step::Allocate(bytes));
        self.next_block += 1;
        self.next_block
    }
    fn attach(&mut self, dst: u8, block: u16) {
        self.steps.push(Step::Attach(dst, block));
    }
    fn init_count(&mut self, block: u16) {
        self.steps.push(Step::InitCount(block));
    }
    fn copy_payload(&mut self, dst: u8, src: u8) {
        self.steps.push(Step::CopyPayload(dst, src));
    }
    fn increment(&mut self, block: u16) {
        self.steps.push(Step::Increment(block));
    }
    fn decrement(&mut self, block: u16) -> bool {
        self.steps.push(Step::Decrement(block));
        self.last.contains(&block)
    }
    fn free(&mut self, block: u16) {
        self.steps.push(Step::Free(block));
    }
    fn zero(&mut self, packet: u8) {
        self.steps.push(Step::Zero(packet));
    }
}

#[test]
fn create_instance_copies_then_counts_then_fills() {
    for size in [0, 1, 5, usize::MAX - 8] {
        let mut h = Recorder {
            sizes: vec![(2, size)],
            ..Recorder::default()
        };
        create_instance(&mut h, 1, 2);
        assert_eq!(
            h.steps,
            [
                Step::Assign(1, 2),
                Step::Allocate(size.wrapping_add(8)),
                Step::Attach(1, 1),
                Step::InitCount(1),
                Step::CopyPayload(1, 2),
            ],
            "size {size}"
        );
    }
}

#[test]
fn create_instance_ignores_an_owned_source_count() {
    let mut h = Recorder {
        sizes: vec![(2, 3)],
        blocks: vec![(2, 40)],
        next_block: 7,
        ..Recorder::default()
    };
    create_instance(&mut h, 1, 2);
    assert_eq!(
        h.steps,
        [
            Step::Assign(1, 2),
            Step::Allocate(11),
            Step::Attach(1, 8),
            Step::InitCount(8),
            Step::CopyPayload(1, 2),
        ]
    );
}

#[test]
fn reference_counts_before_the_copy() {
    let mut h = Recorder {
        blocks: vec![(2, 9)],
        ..Recorder::default()
    };
    reference(&mut h, 1, Some(2));
    assert_eq!(h.steps, [Step::Increment(9), Step::Assign(1, 2)]);

    let mut h = Recorder::default();
    reference(&mut h, 1, Some(2));
    assert_eq!(h.steps, [Step::Assign(1, 2)], "no payload, no count");

    let mut h = Recorder::default();
    reference(&mut h, 1, None);
    assert!(h.steps.is_empty(), "missing source: nothing");
}

#[test]
fn release_frees_on_the_last_reference_then_zeroes() {
    let mut h = Recorder {
        blocks: vec![(1, 4)],
        last: vec![4],
        ..Recorder::default()
    };
    release(&mut h, Some(1));
    assert_eq!(h.steps, [Step::Decrement(4), Step::Free(4), Step::Zero(1)]);

    let mut h = Recorder {
        blocks: vec![(1, 4)],
        ..Recorder::default()
    };
    release(&mut h, Some(1));
    assert_eq!(h.steps, [Step::Decrement(4), Step::Zero(1)], "still shared");

    let mut h = Recorder::default();
    release(&mut h, Some(1));
    assert_eq!(h.steps, [Step::Zero(1)], "no payload: only zeroed");

    let mut h = Recorder::default();
    release(&mut h, None);
    assert!(h.steps.is_empty(), "missing packet: nothing");
}

// ---------------------------------------------------------------------------
// test_encoder_packet_ownership.c, through the exports

const VIDEO: i32 = 1;

fn zeroed() -> encoder_packet {
    // SAFETY: every field of the C struct is valid all-zero.
    unsafe { std::mem::zeroed() }
}

fn assert_metadata(actual: &encoder_packet, expected: &encoder_packet) {
    assert_eq!(actual.size, expected.size);
    assert_eq!(actual.pts, expected.pts);
    assert_eq!(actual.dts, expected.dts);
    assert_eq!(actual.timebase_num, expected.timebase_num);
    assert_eq!(actual.timebase_den, expected.timebase_den);
    assert_eq!(actual.r#type, expected.r#type);
    assert_eq!(actual.keyframe, expected.keyframe);
    assert_eq!(actual.dts_usec, expected.dts_usec);
    assert_eq!(actual.sys_dts_usec, expected.sys_dts_usec);
    assert_eq!(actual.priority, expected.priority);
    assert_eq!(actual.drop_priority, expected.drop_priority);
    assert_eq!(actual.track_idx, expected.track_idx);
    assert_eq!(actual.encoder, expected.encoder);
}

fn bytes_of(p: &encoder_packet) -> Vec<u8> {
    // SAFETY: plain bytes of a repr(C) struct.
    unsafe {
        std::slice::from_raw_parts(ptr::from_ref(p).cast::<u8>(), size_of::<encoder_packet>())
    }
    .to_vec()
}

fn assert_zeroed(p: &encoder_packet) {
    assert!(bytes_of(p).iter().all(|&b| b == 0), "{p:?}");
}

fn references(p: &encoder_packet) -> CLong {
    // SAFETY: an owned payload follows its count.
    unsafe { p.data.cast::<CLong>().sub(1).read_unaligned() }
}

fn allocs() -> CLong {
    // SAFETY: reads the counter.
    unsafe { bnum_allocs() }
}

fn at(p: &encoder_packet, i: usize) -> u8 {
    // SAFETY: `i` is inside the payload.
    unsafe { *p.data.add(i) }
}

fn set(p: &encoder_packet, i: usize, v: u8) {
    // SAFETY: `i` is inside the payload.
    unsafe { *p.data.add(i) = v };
}

fn copies_raw_and_owned_payloads() {
    let baseline = allocs();
    let mut raw = [0x00_u8, 0xff, 0x42, 0x80, 0x01];
    let mut source = encoder_packet {
        data: raw.as_mut_ptr(),
        size: raw.len(),
        pts: -12_345_678_901,
        dts: 9_876_543_210,
        timebase_num: 1001,
        timebase_den: 30000,
        r#type: VIDEO,
        keyframe: true,
        dts_usec: -123_456,
        sys_dts_usec: 987_654,
        priority: 7,
        drop_priority: -3,
        track_idx: 2,
        encoder: ptr::null_mut(),
    };
    // Opaque identity only, never dereferenced.
    source.encoder = (&raw mut source).cast();
    let mut first = zeroed();
    let mut second = zeroed();
    // SAFETY: valid packets; `source` holds `size` bytes.
    unsafe { obs_encoder_packet_create_instance(&raw mut first, &raw const source) };
    assert_metadata(&first, &source);
    assert_eq!(source.data, raw.as_mut_ptr());
    assert_ne!(first.data, raw.as_mut_ptr());
    // SAFETY: an owned payload of `size` bytes.
    assert_eq!(unsafe { std::slice::from_raw_parts(first.data, 5) }, raw);
    assert_eq!(references(&first), 1);
    assert_eq!(allocs(), baseline + 1);

    // Even an already owned source is independently allocated and copied.
    // SAFETY: valid packets.
    unsafe { obs_encoder_packet_create_instance(&raw mut second, &raw const first) };
    assert_metadata(&second, &source);
    assert_ne!(second.data, first.data);
    // SAFETY: an owned payload of `size` bytes.
    assert_eq!(unsafe { std::slice::from_raw_parts(second.data, 5) }, raw);
    assert_eq!(references(&first), 1);
    assert_eq!(references(&second), 1);
    assert_eq!(allocs(), baseline + 2);
    raw[0] = 0x11;
    set(&first, 1, 0x22);
    assert_eq!(at(&first, 0), 0x00);
    assert_eq!(at(&second, 0), 0x00);
    assert_eq!(at(&second, 1), 0xff);
    assert_eq!(raw[1], 0xff);
    // SAFETY: owned packets.
    unsafe { obs_encoder_packet_release(&raw mut first) };
    assert_zeroed(&first);
    assert_eq!(allocs(), baseline + 1);
    assert_eq!(at(&second, 1), 0xff);
    // SAFETY: an owned packet.
    unsafe { obs_encoder_packet_release(&raw mut second) };
    assert_zeroed(&second);
    assert_eq!(allocs(), baseline);
}

fn retains_aliases_until_last_release() {
    let baseline = allocs();
    let mut raw = [0x12_u8, 0x34, 0x56];
    let source = encoder_packet {
        data: raw.as_mut_ptr(),
        size: raw.len(),
        pts: -4,
        track_idx: 5,
        ..zeroed()
    };
    let mut owner = zeroed();
    let mut first = zeroed();
    let mut second = zeroed();
    // SAFETY: valid packets; owned payloads for the references.
    unsafe {
        obs_encoder_packet_create_instance(&raw mut owner, &raw const source);
        obs_encoder_packet_ref(&raw mut first, &raw mut owner);
    }
    assert_eq!(first.data, owner.data);
    assert_metadata(&first, &owner);
    assert_eq!(references(&owner), 2);
    // SAFETY: an owned payload.
    unsafe { obs_encoder_packet_ref(&raw mut second, &raw mut first) };
    assert_eq!(second.data, owner.data);
    assert_metadata(&second, &owner);
    assert_eq!(references(&owner), 3);
    assert_eq!(allocs(), baseline + 1);
    set(&first, 1, 0xab);
    assert_eq!(at(&owner, 1), 0xab);
    assert_eq!(at(&second, 1), 0xab);
    assert_eq!(raw[1], 0x34);
    // SAFETY: each holds a reference.
    unsafe { obs_encoder_packet_release(&raw mut owner) };
    assert_zeroed(&owner);
    assert_eq!(references(&second), 2);
    // SAFETY: as above.
    unsafe { obs_encoder_packet_release(&raw mut second) };
    assert_zeroed(&second);
    assert_eq!(references(&first), 1);
    assert_eq!(at(&first, 1), 0xab);
    assert_eq!(allocs(), baseline + 1);
    // SAFETY: the last reference, then a zeroed packet.
    unsafe { obs_encoder_packet_release(&raw mut first) };
    assert_zeroed(&first);
    assert_eq!(allocs(), baseline);
    // SAFETY: a zeroed packet.
    unsafe { obs_encoder_packet_release(&raw mut first) };
    assert_zeroed(&first);
    assert_eq!(allocs(), baseline);
}

fn empty_payload_still_allocates() {
    let baseline = allocs();
    let mut sentinel = 0xa5_u8;
    // A valid pointer with size zero avoids relying on memcpy(NULL, ..., 0).
    let source = encoder_packet {
        data: &raw mut sentinel,
        size: 0,
        pts: 42,
        ..zeroed()
    };
    let mut owner = zeroed();
    let mut alias = zeroed();
    // SAFETY: valid packets.
    unsafe { obs_encoder_packet_create_instance(&raw mut owner, &raw const source) };
    assert!(!owner.data.is_null());
    assert_metadata(&owner, &source);
    assert_eq!(references(&owner), 1);
    assert_eq!(allocs(), baseline + 1);
    // SAFETY: an owned payload.
    unsafe { obs_encoder_packet_ref(&raw mut alias, &raw mut owner) };
    assert_eq!(alias.data, owner.data);
    assert_eq!(references(&alias), 2);
    assert_eq!(allocs(), baseline + 1);
    // SAFETY: each holds a reference.
    unsafe { obs_encoder_packet_release(&raw mut alias) };
    assert_zeroed(&alias);
    assert_eq!(references(&owner), 1);
    // SAFETY: the last reference.
    unsafe { obs_encoder_packet_release(&raw mut owner) };
    assert_zeroed(&owner);
    assert_eq!(sentinel, 0xa5);
    assert_eq!(allocs(), baseline);
}

fn null_and_dataless_packets() {
    let baseline = allocs();
    let mut destination = zeroed();
    // SAFETY: plain bytes.
    unsafe { ptr::write_bytes(&raw mut destination, 0xa5, 1) };
    let unchanged = bytes_of(&destination);
    // SAFETY: NULL sources are accepted.
    unsafe {
        obs_encoder_packet_ref(&raw mut destination, ptr::null_mut());
    }
    assert_eq!(bytes_of(&destination), unchanged);
    // SAFETY: NULL packets are accepted.
    unsafe {
        obs_encoder_packet_ref(ptr::null_mut(), ptr::null_mut());
        obs_encoder_packet_release(ptr::null_mut());
    }
    assert_eq!(allocs(), baseline);

    // No data means no prefix access, even with nonzero size and metadata.
    // This is characterized, not endorsed as a valid encoded payload.
    let mut source = encoder_packet {
        data: ptr::null_mut(),
        size: 17,
        pts: -9,
        keyframe: true,
        ..zeroed()
    };
    // SAFETY: a packet with NULL data.
    unsafe { obs_encoder_packet_ref(&raw mut destination, &raw mut source) };
    assert!(destination.data.is_null());
    assert_metadata(&destination, &source);
    assert_eq!(allocs(), baseline);
    // SAFETY: NULL data.
    unsafe { obs_encoder_packet_release(&raw mut destination) };
    assert_zeroed(&destination);
    assert_eq!(source.size, 17);
    // SAFETY: NULL data.
    unsafe { obs_encoder_packet_release(&raw mut source) };
    assert_zeroed(&source);
    assert_eq!(allocs(), baseline);
}

/// The four cases of the C test. They read `bnum_allocs()`, so they run
/// alone in a child process.
#[test]
fn test_encoder_packet_ownership_cases() {
    if !super::isolated(concat!(
        module_path!(),
        "::test_encoder_packet_ownership_cases"
    )) {
        return;
    }
    copies_raw_and_owned_payloads();
    retains_aliases_until_last_release();
    empty_payload_still_allocates();
    null_and_dataless_packets();
}
