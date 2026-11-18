//! Tier 2 layout and Tier 3 parity: obs-encoder's packet ownership exports
//! against the original `libobs/obs-encoder-packet-ownership.c.h`
//! (oracle/encoder_packet_ownership.c).
//!
//! Each run applies one random sequence of creates (from a caller's buffer
//! or from another packet), references (also of NULL and of packets with no
//! data), releases (also of NULL), payload writes and metadata changes to
//! four C packets and four Rust packets, and after every step compares each
//! packet field by field: its data as NULL, a caller's buffer or a block
//! (blocks numbered in order of first sight, so aliasing must match), the
//! block's count and bytes, and the step's `bnum_allocs()` delta. At the end
//! every packet is released and both sides must be back at the start count.
//!
//! Excluded, as the C test excludes them: creating from a packet with NULL
//! data (memcpy from NULL) or into the packet it copies (the copy reads the
//! new, uninitialized payload), and referencing a caller's buffer, which has
//! no count in front of it. A create or reference into a packet that still
//! holds a block would leak it, on both sides alike; the sequences release
//! first. Intentional differences: none.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.
#![allow(unsafe_code)]
use obs_c_oracle::encoder_packet as layout;
use obs_encoder::ffi::packet_ownership::{
    encoder_packet, obs_encoder_packet_create_instance, obs_encoder_packet_ref,
    obs_encoder_packet_release,
};
use obs_util::ffi::bmem::bnum_allocs;
use obs_util::platform::CLong;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use std::mem::offset_of;
use std::ptr;

unsafe extern "C" {
    fn oracle_obs_encoder_packet_create_instance(
        dst: *mut encoder_packet,
        src: *const encoder_packet,
    );
    fn oracle_obs_encoder_packet_ref(dst: *mut encoder_packet, src: *mut encoder_packet);
    fn oracle_obs_encoder_packet_release(pkt: *mut encoder_packet);
}

// ---------------------------------------------------------------------------
// Tier 2: the Rust declaration against the C header

#[test]
fn layout_matches_c() {
    // SAFETY: the oracle's layout queries take no arguments.
    unsafe {
        assert_eq!(
            size_of::<encoder_packet>(),
            layout::oracle_encoder_packet_size()
        );
        assert_eq!(
            align_of::<encoder_packet>(),
            layout::oracle_encoder_packet_align()
        );
        assert_eq!(size_of::<CLong>(), layout::oracle_sizeof_long());
        for (rust, c) in [
            (
                offset_of!(encoder_packet, data),
                layout::oracle_encoder_packet_offset_data(),
            ),
            (
                offset_of!(encoder_packet, size),
                layout::oracle_encoder_packet_offset_size(),
            ),
            (
                offset_of!(encoder_packet, pts),
                layout::oracle_encoder_packet_offset_pts(),
            ),
            (
                offset_of!(encoder_packet, dts),
                layout::oracle_encoder_packet_offset_dts(),
            ),
            (
                offset_of!(encoder_packet, timebase_num),
                layout::oracle_encoder_packet_offset_timebase_num(),
            ),
            (
                offset_of!(encoder_packet, timebase_den),
                layout::oracle_encoder_packet_offset_timebase_den(),
            ),
            (
                offset_of!(encoder_packet, r#type),
                layout::oracle_encoder_packet_offset_type(),
            ),
            (
                offset_of!(encoder_packet, keyframe),
                layout::oracle_encoder_packet_offset_keyframe(),
            ),
            (
                offset_of!(encoder_packet, dts_usec),
                layout::oracle_encoder_packet_offset_dts_usec(),
            ),
            (
                offset_of!(encoder_packet, sys_dts_usec),
                layout::oracle_encoder_packet_offset_sys_dts_usec(),
            ),
            (
                offset_of!(encoder_packet, priority),
                layout::oracle_encoder_packet_offset_priority(),
            ),
            (
                offset_of!(encoder_packet, drop_priority),
                layout::oracle_encoder_packet_offset_drop_priority(),
            ),
            (
                offset_of!(encoder_packet, track_idx),
                layout::oracle_encoder_packet_offset_track_idx(),
            ),
            (
                offset_of!(encoder_packet, encoder),
                layout::oracle_encoder_packet_offset_encoder(),
            ),
        ] {
            assert_eq!(rust, c);
        }
    }
}

// ---------------------------------------------------------------------------
// Tier 3

type Create = unsafe extern "C" fn(*mut encoder_packet, *const encoder_packet);
type Ref = unsafe extern "C" fn(*mut encoder_packet, *mut encoder_packet);
type Release = unsafe extern "C" fn(*mut encoder_packet);

#[derive(Clone, Copy)]
struct Api {
    create: Create,
    reference: Ref,
    release: Release,
}

const C: Api = Api {
    create: oracle_obs_encoder_packet_create_instance,
    reference: oracle_obs_encoder_packet_ref,
    release: oracle_obs_encoder_packet_release,
};

const RUST: Api = Api {
    create: obs_encoder_packet_create_instance,
    reference: obs_encoder_packet_ref,
    release: obs_encoder_packet_release,
};

const SLOTS: usize = 4;

/// Every field but `data`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Meta {
    size: usize,
    pts: i64,
    dts: i64,
    timebase: (i32, i32),
    kind: i32,
    keyframe: bool,
    dts_usec: i64,
    sys_dts_usec: i64,
    priority: (i32, i32),
    track_idx: usize,
    encoder: usize,
}

impl Meta {
    fn of(p: &encoder_packet) -> Self {
        Self {
            size: p.size,
            pts: p.pts,
            dts: p.dts,
            timebase: (p.timebase_num, p.timebase_den),
            kind: p.r#type,
            keyframe: p.keyframe,
            dts_usec: p.dts_usec,
            sys_dts_usec: p.sys_dts_usec,
            priority: (p.priority, p.drop_priority),
            track_idx: p.track_idx,
            encoder: p.encoder as usize,
        }
    }

    fn packet(self, data: *mut u8) -> encoder_packet {
        encoder_packet {
            data,
            size: self.size,
            pts: self.pts,
            dts: self.dts,
            timebase_num: self.timebase.0,
            timebase_den: self.timebase.1,
            r#type: self.kind,
            keyframe: self.keyframe,
            dts_usec: self.dts_usec,
            sys_dts_usec: self.sys_dts_usec,
            priority: self.priority.0,
            drop_priority: self.priority.1,
            track_idx: self.track_idx,
            encoder: ptr::without_provenance_mut(self.encoder),
        }
    }
}

#[derive(Debug, Clone)]
enum Op {
    /// Create into `dst` from a caller's buffer of these bytes.
    CreateRaw {
        dst: usize,
        bytes: Vec<u8>,
        meta: Meta,
    },
    /// Create into `dst` from another packet.
    CreateFrom { dst: usize, src: usize },
    /// Reference `src` (NULL for `None`) into `dst`.
    Ref { dst: usize, src: Option<usize> },
    /// `obs_encoder_packet_ref(NULL, NULL)`.
    RefNull,
    /// Release `slot` (NULL for `None`).
    Release { slot: Option<usize> },
    /// Write one payload byte, seen through every alias.
    Write { slot: usize, at: usize, byte: u8 },
    /// Metadata on a packet with no data (`size` without a payload).
    Dataless { slot: usize, meta: Meta },
}

/// What a packet's `data` is.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Data {
    Null,
    /// The `n`th caller's buffer.
    Raw(usize),
    /// The `n`th block seen, its count and its bytes.
    Block(usize, CLong, Vec<u8>),
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    slots: Vec<(Meta, Data)>,
    allocs: CLong,
}

struct Side {
    api: Api,
    slots: [encoder_packet; SLOTS],
    /// The caller's buffers, alive until the run ends.
    raws: Vec<Vec<u8>>,
}

fn zeroed() -> encoder_packet {
    // SAFETY: every field is valid all-zero.
    unsafe { std::mem::zeroed() }
}

fn allocs() -> CLong {
    // SAFETY: reads the counter.
    unsafe { bnum_allocs() }
}

impl Side {
    fn new(api: Api) -> Self {
        Self {
            api,
            slots: [zeroed(); SLOTS],
            raws: Vec::new(),
        }
    }

    fn slot(&mut self, i: usize) -> *mut encoder_packet {
        &raw mut self.slots[i]
    }

    fn is_raw(&self, data: *const u8) -> Option<usize> {
        self.raws.iter().position(|r| ptr::eq(r.as_ptr(), data))
    }

    /// # Safety
    ///
    /// `op` passed [`allowed`] on this side's state.
    unsafe fn apply(&mut self, op: &Op) {
        let api = self.api;
        // SAFETY: the preconditions [`allowed`] checks.
        unsafe {
            match op {
                Op::CreateRaw { dst, bytes, meta } => {
                    self.raws.push(bytes.clone());
                    let data = self.raws.last_mut().unwrap().as_mut_ptr();
                    let src = Meta {
                        size: bytes.len(),
                        ..*meta
                    }
                    .packet(data);
                    (api.create)(self.slot(*dst), &raw const src);
                }
                Op::CreateFrom { dst, src } => {
                    let src = self.slot(*src);
                    (api.create)(self.slot(*dst), src);
                }
                Op::Ref { dst, src } => {
                    let src = src.map_or(ptr::null_mut(), |s| self.slot(s));
                    (api.reference)(self.slot(*dst), src);
                }
                Op::RefNull => (api.reference)(ptr::null_mut(), ptr::null_mut()),
                Op::Release { slot } => {
                    let p = slot.map_or(ptr::null_mut(), |s| self.slot(s));
                    (api.release)(p);
                }
                Op::Write { slot, at, byte } => {
                    let p = self.slots[*slot];
                    *p.data.add(at % p.size) = *byte;
                }
                Op::Dataless { slot, meta } => self.slots[*slot] = meta.packet(ptr::null_mut()),
            }
        }
    }

    fn snapshot(&self, allocs: CLong) -> Snapshot {
        let mut blocks: Vec<*mut u8> = Vec::new();
        let slots = self
            .slots
            .iter()
            .map(|p| {
                let data = if p.data.is_null() {
                    Data::Null
                } else if let Some(n) = self.is_raw(p.data) {
                    Data::Raw(n)
                } else {
                    let n = blocks.iter().position(|b| *b == p.data).unwrap_or_else(|| {
                        blocks.push(p.data);
                        blocks.len() - 1
                    });
                    // SAFETY: a live block: its count, then `size` bytes.
                    let (count, bytes) = unsafe {
                        (
                            p.data.cast::<CLong>().sub(1).read(),
                            std::slice::from_raw_parts(p.data, p.size).to_vec(),
                        )
                    };
                    Data::Block(n, count, bytes)
                };
                (Meta::of(p), data)
            })
            .collect();
        Snapshot { slots, allocs }
    }

    /// Releases every packet: each holds one reference to its block.
    fn finish(&mut self) {
        for i in 0..SLOTS {
            if !self.slots[i].data.is_null() && self.is_raw(self.slots[i].data).is_none() {
                // SAFETY: a packet holding a reference.
                unsafe { (self.api.release)(self.slot(i)) };
            }
        }
    }
}

/// Whether `op` is defined on `side` (see the exclusions above).
fn allowed(side: &Side, op: &Op) -> bool {
    let owned = |i: usize| {
        let d = side.slots[i].data;
        !d.is_null() && side.is_raw(d).is_none()
    };
    let empty = |i: usize| side.slots[i].data.is_null();
    match op {
        Op::CreateRaw { dst, .. } => empty(*dst),
        Op::CreateFrom { dst, src } => dst != src && empty(*dst) && owned(*src),
        Op::Ref { dst, src } => {
            empty(*dst) && src.is_none_or(|s| s != *dst && (owned(s) || empty(s)))
        }
        Op::RefNull | Op::Release { slot: None } => true,
        Op::Release { slot: Some(s) } => owned(*s) || empty(*s),
        Op::Write { slot, .. } => owned(*slot) && side.slots[*slot].size > 0,
        Op::Dataless { slot, .. } => empty(*slot),
    }
}

fn run(ops: &[Op]) {
    let start = allocs();
    let mut c = Side::new(C);
    let mut rust = Side::new(RUST);
    for (i, op) in ops.iter().enumerate() {
        if !allowed(&c, op) {
            assert!(!allowed(&rust, op), "step {i}: {op:?} allowed only in Rust");
            continue;
        }
        let step = |side: &mut Side| {
            let before = allocs();
            // SAFETY: allowed on this side.
            unsafe { side.apply(op) };
            side.snapshot(allocs() - before)
        };
        let cs = step(&mut c);
        let rs = step(&mut rust);
        assert_eq!(
            cs, rs,
            "step {i}: {op:?}\nops: {ops:#?}\nC (left), Rust (right)"
        );
    }
    c.finish();
    rust.finish();
    assert_eq!(allocs(), start, "every block freed\nops: {ops:#?}");
}

fn meta() -> impl Strategy<Value = Meta> {
    (
        (
            0..64_usize,
            any::<i64>(),
            any::<i64>(),
            any::<(i32, i32)>(),
            0..3_i32,
        ),
        (
            any::<bool>(),
            any::<i64>(),
            any::<i64>(),
            any::<(i32, i32)>(),
            0..6_usize,
        ),
        any::<usize>(),
    )
        .prop_map(
            |((size, pts, dts, timebase, kind), (keyframe, dts_usec, sys, priority, track), e)| {
                Meta {
                    size,
                    pts,
                    dts,
                    timebase,
                    kind,
                    keyframe,
                    dts_usec,
                    sys_dts_usec: sys,
                    priority,
                    track_idx: track,
                    encoder: e,
                }
            },
        )
}

fn slot() -> impl Strategy<Value = usize> {
    0..SLOTS
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => (slot(), prop::collection::vec(any::<u8>(), 0..12), meta())
            .prop_map(|(dst, bytes, meta)| Op::CreateRaw { dst, bytes, meta }),
        3 => (slot(), slot()).prop_map(|(dst, src)| Op::CreateFrom { dst, src }),
        4 => (slot(), prop::option::weighted(0.9, slot()))
            .prop_map(|(dst, src)| Op::Ref { dst, src }),
        1 => Just(Op::RefNull),
        4 => prop::option::weighted(0.9, slot()).prop_map(|slot| Op::Release { slot }),
        2 => (slot(), any::<usize>(), any::<u8>())
            .prop_map(|(slot, at, byte)| Op::Write { slot, at, byte }),
        1 => (slot(), meta()).prop_map(|(slot, meta)| Op::Dataless { slot, meta }),
    ]
}

/// The C test's cases as sequences, then random ones. They read
/// `bnum_allocs()`, so they run alone in a child process.
#[test]
fn parity_sequences() {
    if !super::isolated(concat!(module_path!(), "::parity_sequences")) {
        return;
    }
    let m = Meta {
        size: 0,
        pts: -4,
        dts: 0,
        timebase: (0, 0),
        kind: 0,
        keyframe: false,
        dts_usec: 0,
        sys_dts_usec: 0,
        priority: (0, 0),
        track_idx: 5,
        encoder: 0,
    };
    let raw = |dst, bytes: &[u8]| Op::CreateRaw {
        dst,
        bytes: bytes.to_vec(),
        meta: m,
    };
    let rel = |s| Op::Release { slot: Some(s) };
    for ops in [
        // copies_raw_and_owned_payloads
        vec![
            raw(0, &[0x00, 0xff, 0x42, 0x80, 0x01]),
            Op::CreateFrom { dst: 1, src: 0 },
            Op::Write {
                slot: 0,
                at: 1,
                byte: 0x22,
            },
            rel(0),
            rel(1),
        ],
        // retains_aliases_until_last_release
        vec![
            raw(0, &[0x12, 0x34, 0x56]),
            Op::Ref {
                dst: 1,
                src: Some(0),
            },
            Op::Ref {
                dst: 2,
                src: Some(1),
            },
            Op::Write {
                slot: 1,
                at: 1,
                byte: 0xab,
            },
            rel(0),
            rel(2),
            rel(1),
            rel(1),
        ],
        // empty_payload_still_allocates
        vec![
            raw(0, &[]),
            Op::Ref {
                dst: 1,
                src: Some(0),
            },
            rel(1),
            rel(0),
        ],
        // null_and_dataless_packets
        vec![
            Op::Ref { dst: 0, src: None },
            Op::RefNull,
            Op::Release { slot: None },
            Op::Dataless {
                slot: 1,
                meta: Meta {
                    size: 17,
                    pts: -9,
                    keyframe: true,
                    ..m
                },
            },
            Op::Ref {
                dst: 0,
                src: Some(1),
            },
            rel(0),
            rel(1),
        ],
    ] {
        run(&ops);
    }
    let mut runner = TestRunner::new(Config {
        cases: 1024,
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(&prop::collection::vec(op(), 0..40), |ops| {
            run(&ops);
            Ok(())
        })
        .unwrap();
}
