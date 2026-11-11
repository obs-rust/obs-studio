#include "libobs/build/libobs.pch.h"

void obs_encoder_packet_create_instance(struct encoder_packet *dst, const struct encoder_packet *src)
{
	long *p_refs;

	*dst = *src;
	p_refs = bmalloc(src->size + sizeof(long));
	dst->data = (void *)(p_refs + 1);
	*p_refs = 1;
	memcpy(dst->data, src->data, src->size);
}

void obs_encoder_packet_ref(struct encoder_packet *dst, struct encoder_packet *src)
{
	if (!src)
		return;

	if (src->data) {
		long *p_refs = ((long *)src->data) - 1;
		os_atomic_inc_long(p_refs);
	}

	*dst = *src;
}

void obs_encoder_packet_release(struct encoder_packet *pkt)
{
	if (!pkt)
		return;

	if (pkt->data) {
		long *p_refs = ((long *)pkt->data) - 1;
		if (os_atomic_dec_long(p_refs) == 0)
			bfree(p_refs);
	}

	memset(pkt, 0, sizeof(struct encoder_packet));
}
