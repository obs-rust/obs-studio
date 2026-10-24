#pragma once

/* Waiting for an ingest-list refresh (service-ingest and dacast).
 *
 * The refresh runs on the file-updater thread; the caller (the output's
 * connect thread via rtmp_common_url, or the UI thread via the
 * *_ingests_refresh proc handlers) blocks until the refresh attempt ends or
 * the timeout passes. The refresh signals a manual-reset event when its
 * attempt ends, so the waiter wakes at once instead of at a polling tick.
 *
 * If that event cannot be created, it stays NULL: the refresh still runs,
 * but there is nothing to wait on, so the caller goes on at once with the
 * ingest list it has (the default one, or the cached one). */

#include <util/threading.h>
#include <util/platform.h>

struct ingest_refresh_wait {
	os_event_t *done;
};

static inline void ingest_refresh_wait_init(struct ingest_refresh_wait *w)
{
	if (os_event_init(&w->done, OS_EVENT_TYPE_MANUAL) != 0)
		w->done = NULL;
}

static inline void ingest_refresh_wait_free(struct ingest_refresh_wait *w)
{
	if (w->done) {
		os_event_destroy(w->done);
		w->done = NULL;
	}
}

/* Called before a new refresh starts. */
static inline void ingest_refresh_wait_reset(struct ingest_refresh_wait *w)
{
	if (w->done)
		os_event_reset(w->done);
}

/* Called by the refresh when its attempt ends, whatever the outcome: a
 * download that did not parse still ends the wait. */
static inline void ingest_refresh_wait_complete(struct ingest_refresh_wait *w, bool success)
{
	(void)success;
	if (w->done)
		os_event_signal(w->done);
}

/* Whether a caller has to wait: only when no ingest list is available yet
 * (none from the cache, none from a finished refresh). */
static inline bool ingest_refresh_needs_wait(bool refreshed, bool loaded)
{
	return !refreshed && !loaded;
}

/* Blocks until the refresh attempt ends or timeout_ms passes. Returns true
 * when the attempt ended. */
static inline bool ingest_refresh_wait_for(struct ingest_refresh_wait *w, unsigned long timeout_ms)
{
	return w->done && os_event_timedwait(w->done, timeout_ms) == 0;
}
