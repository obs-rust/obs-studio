/* Test-only oracle: the original libobs/obs-encoder-packet-ownership.c.h,
 * unmodified, with its three functions renamed oracle_* so they link next
 * to obs-encoder's exports. bmalloc, bfree and the atomics keep their
 * libobs names, so both sides use the same allocator and count. */
#include <string.h>

#include "obs.h"
#include "util/bmem.h"
#include "util/threading.h"

void oracle_obs_encoder_packet_create_instance(struct encoder_packet *dst, const struct encoder_packet *src);
void oracle_obs_encoder_packet_ref(struct encoder_packet *dst, struct encoder_packet *src);
void oracle_obs_encoder_packet_release(struct encoder_packet *pkt);

#define obs_encoder_packet_create_instance oracle_obs_encoder_packet_create_instance
#define obs_encoder_packet_ref oracle_obs_encoder_packet_ref
#define obs_encoder_packet_release oracle_obs_encoder_packet_release
#include "encoder_packet_ownership.inc"
#undef obs_encoder_packet_release
#undef obs_encoder_packet_ref
#undef obs_encoder_packet_create_instance
