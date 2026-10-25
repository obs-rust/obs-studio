/* Test-only oracle: the original libobs/obs-output-stop-stats.c.h,
 * unmodified, with its function renamed to oracle_* so it can link next to
 * the Rust shim, plus the layout of its struct as the C sees it. Its only
 * divisions are double, so no input traps on any host; total - dropped is
 * int arithmetic, undefined where it overflows, so callers keep it in
 * range. */
#include <stddef.h>

/* log_frame_info() starts both double percentages from 0.0f, as
 * obs-output.c always did. libobs builds without -Wdouble-promotion; the
 * oracle has it, and clang flags the float constant. */
#if defined(__clang__) || defined(__GNUC__)
#pragma GCC diagnostic ignored "-Wdouble-promotion"
#endif

#define obs_output_stop_stats oracle_obs_output_stop_stats

#include "obs-output-stop-stats.c.h"

#undef obs_output_stop_stats

/* The define renames the struct's tag with the function (they share the
 * name). In the order of rust/obs-c-oracle/src/output_stop_stats.rs. */
enum {
	ORACLE_OUTPUT_STOP_STATS_LAYOUT_COUNT = 12,
};

size_t oracle_output_stop_stats_layout(size_t which);

size_t oracle_output_stop_stats_layout(size_t which)
{
	const size_t values[ORACLE_OUTPUT_STOP_STATS_LAYOUT_COUNT] = {
		sizeof(struct oracle_obs_output_stop_stats),
		_Alignof(struct oracle_obs_output_stop_stats),
		offsetof(struct oracle_obs_output_stop_stats, drawn),
		offsetof(struct oracle_obs_output_stop_stats, lagged),
		offsetof(struct oracle_obs_output_stop_stats, percentage_lagged),
		offsetof(struct oracle_obs_output_stop_stats, percentage_dropped),
		offsetof(struct oracle_obs_output_stop_stats, frames_output),
		offsetof(struct oracle_obs_output_stop_stats, output_attempted),
		offsetof(struct oracle_obs_output_stop_stats, frames_drawn),
		offsetof(struct oracle_obs_output_stop_stats, drawn_attempted),
		offsetof(struct oracle_obs_output_stop_stats, lag_line),
		offsetof(struct oracle_obs_output_stop_stats, drop_line),
	};
	return which < ORACLE_OUTPUT_STOP_STATS_LAYOUT_COUNT ? values[which] : (size_t)-1;
}
