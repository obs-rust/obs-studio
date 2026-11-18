//! Encoder packet ownership: an owned payload sits right after its native
//! `long` reference count, in one libobs allocation.
//!
//! The three bodies run their primitive steps in the original C order; the
//! ABI boundary supplies the steps (struct copy, bmem, the atomic count,
//! zeroing) through [`PacketHost`].

/// The primitive steps of `obs-encoder-packet-ownership.c.h`.
pub trait PacketHost {
    /// A `struct encoder_packet` location.
    type Packet: Copy;
    /// The reference count block in front of an owned payload.
    type Block: Copy;

    /// `sizeof(long)`, the size of the count in front of the payload.
    fn header_size(&self) -> usize;
    /// `packet->size`.
    fn size(&self, packet: Self::Packet) -> usize;
    /// `((long *)packet->data) - 1`, or `None` for a NULL `data`.
    fn block(&self, packet: Self::Packet) -> Option<Self::Block>;
    /// `*dst = *src`: every field, payload pointer included.
    fn assign(&mut self, dst: Self::Packet, src: Self::Packet);
    /// `bmalloc(bytes)`.
    fn allocate(&mut self, bytes: usize) -> Self::Block;
    /// `dst->data = (void *)(block + 1)`.
    fn attach(&mut self, dst: Self::Packet, block: Self::Block);
    /// `*block = 1`.
    fn init_count(&mut self, block: Self::Block);
    /// `memcpy(dst->data, src->data, src->size)`.
    fn copy_payload(&mut self, dst: Self::Packet, src: Self::Packet);
    /// `os_atomic_inc_long(block)`.
    fn increment(&mut self, block: Self::Block);
    /// `os_atomic_dec_long(block)`: true when the count reached zero.
    fn decrement(&mut self, block: Self::Block) -> bool;
    /// `bfree(block)`.
    fn free(&mut self, block: Self::Block);
    /// `memset(packet, 0, sizeof(struct encoder_packet))`.
    fn zero(&mut self, packet: Self::Packet);
}

/// `obs_encoder_packet_create_instance`: an independent owned copy of `src`
/// with a count of one, whatever `src`'s payload is (raw or already owned).
pub fn create_instance<H: PacketHost>(host: &mut H, dst: H::Packet, src: H::Packet) {
    host.assign(dst, src);
    // `src->size + sizeof(long)`, unsigned as in C.
    let bytes = host.size(src).wrapping_add(host.header_size());
    let block = host.allocate(bytes);
    host.attach(dst, block);
    host.init_count(block);
    host.copy_payload(dst, src);
}

/// `obs_encoder_packet_ref`: alias `src` into `dst`, retaining its payload.
/// A missing `src` is a no-op; a NULL payload is copied without a count.
pub fn reference<H: PacketHost>(host: &mut H, dst: H::Packet, src: Option<H::Packet>) {
    let Some(src) = src else {
        return;
    };
    if let Some(block) = host.block(src) {
        host.increment(block);
    }
    host.assign(dst, src);
}

/// `obs_encoder_packet_release`: drop one reference, free the block on the
/// last one, then zero the whole packet. A missing packet is a no-op.
pub fn release<H: PacketHost>(host: &mut H, packet: Option<H::Packet>) {
    let Some(packet) = packet else {
        return;
    };
    if let Some(block) = host.block(packet)
        && host.decrement(block)
    {
        host.free(block);
    }
    host.zero(packet);
}
