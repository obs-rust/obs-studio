#pragma once

#include <stdbool.h>
#include <util/threading.h>

/* Cuts libVLC's audio waits short. vlcs_audio_play holds each block until it
 * is nearly due and vlcs_audio_drain waits for the end of a track, each for
 * up to seconds on libVLC's audio thread, and libVLC's stop waits for that
 * thread. Stop, restart, next/previous, settings updates and destruction
 * open the gate so a pending wait ends at once; libVLC closes it again when
 * it opens the next item, so the new item's waits work.
 *
 * libVLC reports an item opening on its own event thread, also at a playlist
 * boundary, so that report can arrive just after a Stop. It must not close
 * the gate then: libVLC's stop would wait behind a re-armed wait for up to
 * seconds (#1011). A Stop keeps the gate open until OBS starts playback. */
struct vlc_drain_gate {
	os_event_t *open;
	volatile bool stop_requested;
};

static inline bool vlc_drain_gate_init(struct vlc_drain_gate *g)
{
	return os_event_init(&g->open, OS_EVENT_TYPE_MANUAL) == 0;
}

static inline void vlc_drain_gate_free(struct vlc_drain_gate *g)
{
	if (g->open)
		os_event_destroy(g->open);
	g->open = NULL;
}

/* Playback is being stopped: end any wait, and keep it ended. */
static inline void vlc_drain_gate_stop(struct vlc_drain_gate *g)
{
	os_atomic_set_bool(&g->stop_requested, true);
	if (g->open)
		os_event_signal(g->open);
}

/* The item is changing (next, previous): end any wait. */
static inline void vlc_drain_gate_abort(struct vlc_drain_gate *g)
{
	if (g->open)
		os_event_signal(g->open);
}

/* OBS starts playback (play, restart, update): the next item that opens may
 * close the gate again. */
static inline void vlc_drain_gate_play(struct vlc_drain_gate *g)
{
	os_atomic_set_bool(&g->stop_requested, false);
}

/* libVLC opened an item (its event thread, also at a playlist boundary).
 * Closes the gate for the item's waits, unless a Stop is pending. Returns
 * false when a Stop is pending: libVLC started the item after (or while) OBS
 * stopped playback, and it is about to be stopped. */
static inline bool vlc_drain_gate_opened(struct vlc_drain_gate *g)
{
	if (os_atomic_load_bool(&g->stop_requested))
		return false;
	if (g->open)
		os_event_reset(g->open);
	return true;
}

/* A Stop is pending: OBS stopped playback and has not started it again. */
static inline bool vlc_drain_gate_stopping(struct vlc_drain_gate *g)
{
	return os_atomic_load_bool(&g->stop_requested);
}

/* Waits up to `ms`, less if the gate is opened. */
static inline void vlc_drain_gate_wait(struct vlc_drain_gate *g, unsigned long ms)
{
	if (g->open)
		os_event_timedwait(g->open, ms);
}
