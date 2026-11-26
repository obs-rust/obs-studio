/* Tier 3: the unchanged extracted obs_source_output_cea708()
 * (libobs/obs-source-output-cea708.c.h) over the real source caption
 * fixtures of source_caption_callbacks.c, against the Rust export. The
 * callbacks and their recording live in the parity test. */
#define _NO_CRT_STDIO_INLINE
#include <stddef.h>
#include "libobs/build/libobs.pch.h"

void oracle_obs_source_output_cea708(obs_source_t *source, const struct obs_source_cea_708 *captions);
#define obs_source_output_cea708 oracle_obs_source_output_cea708
#include "source-output-cea708.inc"
#undef obs_source_output_cea708

void oracle_cea708_set_destroying(obs_source_t *source, long destroying);
void oracle_cea708_set_destroying(obs_source_t *source, long destroying)
{
	os_atomic_set_long(&source->destroying, destroying);
}

/* Appends without the mutex, as a callback that grows the list it is
 * called from does (the mutex is already held by the dispatch). */
void oracle_cea708_push(obs_source_t *source, obs_source_caption_t callback, void *param);
void oracle_cea708_push(obs_source_t *source, obs_source_caption_t callback, void *param)
{
	struct caption_cb_info info = {callback, param};
	da_push_back(source->caption_cb_list, &info);
}
