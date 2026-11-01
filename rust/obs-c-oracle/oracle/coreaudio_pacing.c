/* Test-only oracle: the original
 * libobs/audio-monitoring/osx/coreaudio-pacing.c.h, unmodified, with its seven
 * functions renamed to oracle_* so they can link next to the Rust shims, plus
 * its constant as the C sees it. It calls nothing outside the file but
 * fabsf() (close_float()). */
#include <stddef.h>
#include <stdint.h>

#define coreaudio_monitor_buffer_size oracle_coreaudio_monitor_buffer_size
#define coreaudio_monitor_wait_size oracle_coreaudio_monitor_wait_size
#define coreaudio_monitor_can_fill oracle_coreaudio_monitor_can_fill
#define coreaudio_monitor_needs_volume oracle_coreaudio_monitor_needs_volume
#define coreaudio_monitor_wait_over oracle_coreaudio_monitor_wait_over
#define coreaudio_monitor_all_back oracle_coreaudio_monitor_all_back
#define coreaudio_monitor_recover_failed_enqueue oracle_coreaudio_monitor_recover_failed_enqueue

#include "audio-monitoring/osx/coreaudio-pacing.c.h"

size_t oracle_coreaudio_monitor_buffers(void);

size_t oracle_coreaudio_monitor_buffers(void)
{
	return COREAUDIO_MONITOR_BUFFERS;
}
