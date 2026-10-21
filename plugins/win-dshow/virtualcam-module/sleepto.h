#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

extern uint64_t gettime_100ns(void);
/* Waits toward `time_target` and returns true when it had not already passed on
 * entry. It does not promise the target was reached: the sleep stops within a
 * timer tick of it, up to ~2 ms early or late depending on the host's timer
 * granularity (obs-rust/obs-studio#1279). */
extern bool sleepto_100ns(uint64_t time_target);

#ifdef __cplusplus
}
#endif
