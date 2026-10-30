/* Test-only oracle: the original libobs/obs-output-caption.c.h, unmodified,
 * with its three functions renamed to oracle_* so they can link next to the
 * Rust shims. The parity test never passes an HEVC packet shorter than what
 * obs_output_caption_hevc_sei_header() reads (the C reads past it). */
#include <stdbool.h>
#include <stdint.h>

#define obs_output_caption_begin oracle_obs_output_caption_begin
#define obs_output_caption_hevc_sei_header oracle_obs_output_caption_hevc_sei_header
#define obs_output_caption_cc_keep oracle_obs_output_caption_cc_keep

#include "obs-output-caption.c.h"

#undef obs_output_caption_cc_keep
#undef obs_output_caption_hevc_sei_header
#undef obs_output_caption_begin
