//! obs-scene: the scene item order rules
//! (`libobs/obs-scene-item-order.c.h`). A drag in the source tree calls
//! `obs_scene_reorder_items2()`, which checks the whole order against the
//! scene's and asks each group entry for its run; Move Up/Down/Top/Bottom
//! asks one anchor. Each side checks a 16-entry order (two groups of four,
//! one item moved), walks its group runs one entry at a time, and asks the anchor of every
//! movement for every neighbour pair, folded into a checksum both sides must
//! match. The Rust check also validates the order (two sorted copies of its
//! items, the fix of #2895), which the C does not: about 95 ns against 37 ns
//! on x86_64 Linux, once per drag. Neither side allocates through bmalloc.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::scene_item_order::{
    OracleOrderInfo, oracle_obs_scene_item_order_anchor, oracle_obs_scene_item_order_in_group_run,
    oracle_obs_scene_item_order_reorder_check,
};
use obs_scene::ffi::scene_item_order::{
    OrderInfo, obs_scene_item_order_anchor, obs_scene_item_order_in_group_run,
    obs_scene_item_order_reorder_check,
};
use std::ffi::c_void;
use std::hint::black_box;

static SLOTS: [u8; 16] = [0; 16];

fn item(i: usize) -> *const c_void {
    std::ptr::from_ref(&SLOTS[i]).cast()
}

/// Items 0-5 at the top level, group 6 holding 7-10, group 11 holding
/// 12-15; `moved` puts item 0 last.
fn scene(moved: bool) -> Vec<OrderInfo> {
    let mut v = Vec::new();
    let top = |i| OrderInfo {
        group: std::ptr::null(),
        item: item(i),
    };
    let sub = |g, i| OrderInfo {
        group: item(g),
        item: item(i),
    };
    for i in 0..6 {
        v.push(top(i));
    }
    v.push(top(6));
    for i in 7..11 {
        v.push(sub(6, i));
    }
    v.push(top(11));
    for i in 12..16 {
        v.push(sub(11, i));
    }
    if moved {
        let first = v.remove(0);
        v.push(first);
    }
    v
}

fn oracle(v: &[OrderInfo]) -> Vec<OracleOrderInfo> {
    v.iter()
        .map(|p| OracleOrderInfo {
            group: p.group,
            item: p.item,
        })
        .collect()
}

fn rust_side(current: &[OrderInfo], order: &[OrderInfo]) -> u64 {
    // SAFETY: both arrays hold their length in pairs.
    let check = unsafe {
        obs_scene_item_order_reorder_check(
            black_box(current.as_ptr()),
            current.len(),
            black_box(order.as_ptr()),
            order.len(),
        )
    };
    let mut acc = u64::from(check.unsigned_abs());
    for head in [6, 11] {
        let head = head - 1;
        let mut end = head + 1;
        // SAFETY: as above, head < len; an index past the end is not read.
        while unsafe { obs_scene_item_order_in_group_run(order.as_ptr(), order.len(), head, end) } {
            end += 1;
        }
        acc = acc.wrapping_mul(31).wrapping_add(end as u64);
    }
    for movement in 0..4 {
        for flags in 0..4 {
            let a = obs_scene_item_order_anchor(black_box(movement), flags & 1 != 0, flags & 2 != 0);
            acc = acc.wrapping_mul(31).wrapping_add(u64::from(a.unsigned_abs()));
        }
    }
    acc
}

fn c_side(current: &[OracleOrderInfo], order: &[OracleOrderInfo]) -> u64 {
    // SAFETY: both arrays hold their length in pairs; the oracle keeps no
    // state.
    let check = unsafe {
        oracle_obs_scene_item_order_reorder_check(
            black_box(current.as_ptr()),
            current.len(),
            black_box(order.as_ptr()),
            order.len(),
        )
    };
    let mut acc = u64::from(check.unsigned_abs());
    for head in [6, 11] {
        let head = head - 1;
        let mut end = head + 1;
        // SAFETY: as above, head < len; an index past the end is not read.
        while unsafe { oracle_obs_scene_item_order_in_group_run(order.as_ptr(), order.len(), head, end) } {
            end += 1;
        }
        acc = acc.wrapping_mul(31).wrapping_add(end as u64);
    }
    for movement in 0..4 {
        for flags in 0..4 {
            // SAFETY: plain values in, an int out.
            let a = unsafe {
                oracle_obs_scene_item_order_anchor(black_box(movement), flags & 1 != 0, flags & 2 != 0)
            };
            acc = acc.wrapping_mul(31).wrapping_add(u64::from(a.unsigned_abs()));
        }
    }
    acc
}

pub fn benchmarks(criterion: &mut Criterion) {
    let current = scene(false);
    let order = scene(true);
    let (c_current, c_order) = (oracle(&current), oracle(&order));
    pair(
        criterion,
        "scene_item_order/reorder2/16",
        || rust_side(&current, &order),
        || c_side(&c_current, &c_order),
    );
}
