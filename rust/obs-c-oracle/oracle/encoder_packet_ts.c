/* Test-only oracle: the original libobs/obs-encoder-packet-ts.c.h,
 * unmodified, with its five functions renamed to oracle_* so they can link
 * next to the Rust shims, plus the layout of its struct and its constant as
 * the C sees them. obs_encoder_packet_ts_dts_usec() divides: the parity test
 * never passes the inputs where C is undefined (a timebase_den of 0,
 * dts * 1000000 beyond int64_t, INT64_MIN / -1). */
#include <stddef.h>
#include <stdint.h>

#define obs_encoder_packet_ts_timebase oracle_obs_encoder_packet_ts_timebase
#define obs_encoder_packet_ts_dts_usec oracle_obs_encoder_packet_ts_dts_usec
#define obs_encoder_packet_ts_system oracle_obs_encoder_packet_ts_system
#define obs_encoder_packet_ts_paused oracle_obs_encoder_packet_ts_paused
#define obs_encoder_packet_ts_find oracle_obs_encoder_packet_ts_find

#include "obs-encoder-packet-ts.c.h"

#undef obs_encoder_packet_ts_find
#undef obs_encoder_packet_ts_paused
#undef obs_encoder_packet_ts_system
#undef obs_encoder_packet_ts_dts_usec
#undef obs_encoder_packet_ts_timebase

/* In the order of rust/obs-c-oracle/src/encoder_packet_ts.rs. */
enum {
	ORACLE_ENCODER_PACKET_TS_LAYOUT_COUNT = 9,
};

size_t oracle_encoder_packet_ts_layout(size_t which);

size_t oracle_encoder_packet_ts_layout(size_t which)
{
	const size_t values[ORACLE_ENCODER_PACKET_TS_LAYOUT_COUNT] = {
		sizeof(struct obs_encoder_packet_timebase),
		_Alignof(struct obs_encoder_packet_timebase),
		offsetof(struct obs_encoder_packet_timebase, num),
		offsetof(struct obs_encoder_packet_timebase, den),
		sizeof(struct encoder_packet_time),
		_Alignof(struct encoder_packet_time),
		offsetof(struct encoder_packet_time, pts),
		offsetof(struct encoder_packet_time, cts),
		OBS_ENCODER_PACKET_TS_NOT_FOUND,
	};
	return which < ORACLE_ENCODER_PACKET_TS_LAYOUT_COUNT ? values[which] : (size_t)-1;
}
