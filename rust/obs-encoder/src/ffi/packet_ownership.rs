//! `struct encoder_packet` locations, the native `long` count and libobs's
//! bmem allocator behind the safe [`crate::packet_ownership`] steps.
use crate::packet_ownership::{self, PacketHost};
use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::Ordering::SeqCst;
use obs_util::ffi::bmem::{bfree, bmalloc};
use obs_util::platform::{CLong, CLongAtomic};

/// `struct encoder_packet` of `libobs/obs-encoder.h`; the layout test checks
/// every field against the C header.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct encoder_packet {
    pub data: *mut u8,
    pub size: usize,
    pub pts: i64,
    pub dts: i64,
    pub timebase_num: i32,
    pub timebase_den: i32,
    /// `enum obs_encoder_type`.
    pub r#type: core::ffi::c_int,
    pub keyframe: bool,
    pub dts_usec: i64,
    pub sys_dts_usec: i64,
    pub priority: core::ffi::c_int,
    pub drop_priority: core::ffi::c_int,
    pub track_idx: usize,
    /// `obs_encoder_t *`.
    pub encoder: *mut c_void,
}

/// Raw packet locations. Every method's caller guarantees what the matching
/// C statement needs: a valid packet, and a non-NULL `data` that points just
/// past a live count block.
struct Native;

impl PacketHost for Native {
    type Packet = *mut encoder_packet;
    type Block = *mut CLong;

    fn header_size(&self) -> usize {
        size_of::<CLong>()
    }
    fn size(&self, packet: Self::Packet) -> usize {
        // SAFETY: a valid packet (caller contract).
        unsafe { (*packet).size }
    }
    fn block(&self, packet: Self::Packet) -> Option<Self::Block> {
        // SAFETY: a valid packet; a non-NULL payload follows its count.
        let data = unsafe { (*packet).data };
        // SAFETY: an owned payload starts one `long` into its block.
        (!data.is_null()).then(|| unsafe { data.cast::<CLong>().sub(1) })
    }
    fn assign(&mut self, dst: Self::Packet, src: Self::Packet) {
        // SAFETY: valid packets; the byte copy allows `dst == src`, as C's
        // struct assignment does.
        unsafe {
            ptr::copy(
                src.cast::<u8>(),
                dst.cast::<u8>(),
                size_of::<encoder_packet>(),
            )
        };
    }
    fn allocate(&mut self, bytes: usize) -> Self::Block {
        // SAFETY: bmalloc never returns NULL (it aborts), and its blocks are
        // aligned for `long`.
        unsafe { bmalloc(bytes) }.cast()
    }
    fn attach(&mut self, dst: Self::Packet, block: Self::Block) {
        // SAFETY: a valid packet; the block holds the count and the payload.
        unsafe { (*dst).data = block.add(1).cast() };
    }
    fn init_count(&mut self, block: Self::Block) {
        // SAFETY: a fresh block, not yet shared.
        unsafe { block.write(1) };
    }
    fn copy_payload(&mut self, dst: Self::Packet, src: Self::Packet) {
        // SAFETY: valid packets; `src` holds `size` readable bytes and `dst`
        // a fresh block of `size` writable bytes. The original memcpy of zero
        // bytes reads nothing, so it is skipped rather than given a NULL.
        unsafe {
            let size = (*src).size;
            if size > 0 {
                ptr::copy((*src).data, (*dst).data, size);
            }
        }
    }
    fn increment(&mut self, block: Self::Block) {
        // SAFETY: a live count, shared only through atomic operations.
        unsafe { CLongAtomic::from_ptr(block) }.fetch_add(1, SeqCst);
    }
    fn decrement(&mut self, block: Self::Block) -> bool {
        // SAFETY: a live count, shared only through atomic operations.
        let previous = unsafe { CLongAtomic::from_ptr(block) }.fetch_sub(1, SeqCst);
        previous.wrapping_sub(1) == 0
    }
    fn free(&mut self, block: Self::Block) {
        // SAFETY: the last reference to a bmalloc block.
        unsafe { bfree(block.cast()) };
    }
    fn zero(&mut self, packet: Self::Packet) {
        // SAFETY: a valid packet; padding included, as memset clears it.
        unsafe { ptr::write_bytes(packet.cast::<u8>(), 0, size_of::<encoder_packet>()) };
    }
}

/// Hidden in libobs (`obs-internal.h`).
///
/// # Safety
///
/// `dst` and `src` are valid packets (they may be the same one), and `src`'s
/// `data` is readable for `size` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_packet_create_instance(
    dst: *mut encoder_packet,
    src: *const encoder_packet,
) {
    packet_ownership::create_instance(&mut Native, dst, src.cast_mut());
}

/// # Safety
///
/// `src` is NULL, or a valid packet whose `data` is NULL or an owned payload
/// (from [`obs_encoder_packet_create_instance`]); then `dst` is a valid
/// packet.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_packet_ref(
    dst: *mut encoder_packet,
    src: *mut encoder_packet,
) {
    packet_ownership::reference(&mut Native, dst, (!src.is_null()).then_some(src));
}

/// # Safety
///
/// `packet` is NULL, or a valid packet whose `data` is NULL or an owned
/// payload holding one of its references.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_packet_release(packet: *mut encoder_packet) {
    packet_ownership::release(&mut Native, (!packet.is_null()).then_some(packet));
}
