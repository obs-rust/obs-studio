#pragma once

#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Settings > Output, Advanced: which encoder the encoder lists select. The
 * lists hold the encoders this build has (the streaming list only those the
 * service takes), but a profile can name one they do not offer: it was made
 * on another machine (an AMD encoder in a profile copied to Linux), or the
 * encoder's plugin is gone. Kept free of Qt and libobs so it can be unit
 * tested. */

/* The video encoder Advanced output falls back to for `id`: the same vendor's
 * H.264 encoder for its HEVC and AV1 ones, x264 for the rest. */
const char *adv_video_encoder_fallback(const char *id);

/* Whether the list offers encoder `id`. */
typedef bool (*encoder_list_has_fn)(const char *id, void *param);

/* The encoder the list selects for the profile's `configured` one: itself if
 * the list offers it, else the first of `fallbacks` (null-terminated) that
 * the list offers, else NULL (the caller selects the list's first entry). */
const char *encoder_list_select(const char *configured, const char *const *fallbacks, encoder_list_has_fn has,
				void *param);

#ifdef __cplusplus
}
#endif
