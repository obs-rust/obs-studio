/*
 * Copyright (c) 2023 Lain Bailey <lain@obsproject.com>
 *
 * Permission to use, copy, modify, and distribute this software for any
 * purpose with or without fee is hereby granted, provided that the above
 * copyright notice and this permission notice appear in all copies.
 *
 * THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
 * WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
 * ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
 * WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
 * ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
 * OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
 */

#include "build/libobs-platform.pch.h"

static struct {
	struct timespec ts;
	bool ts_valid;
	uint64_t timestamp;
} timespec_offset = {0};

static void init_timespec_offset(void)
{
	timespec_offset.ts_valid = timespec_get(&timespec_offset.ts, TIME_UTC) == TIME_UTC;
	timespec_offset.timestamp = os_gettime_ns();
}

struct timespec *os_nstime_to_timespec(uint64_t timestamp, struct timespec *storage)
{
	static pthread_once_t once = PTHREAD_ONCE_INIT;
	pthread_once(&once, init_timespec_offset);

	if (!storage || !timespec_offset.ts_valid) {
		return NULL;
	}

	*storage = timespec_offset.ts;

	static const int64_t nsecs_per_sec = 1000000000;

	int64_t nsecs = 0;
	int64_t secs = 0;
	if (timestamp >= timespec_offset.timestamp) {
		uint64_t offset = timestamp - timespec_offset.timestamp;
		nsecs = storage->tv_nsec + offset % nsecs_per_sec;
		secs = storage->tv_sec + offset / nsecs_per_sec;
	} else {
		uint64_t offset = timespec_offset.timestamp - timestamp;
		int64_t nsec_offset = offset % nsecs_per_sec;
		int64_t sec_offset = offset / nsecs_per_sec;
		int64_t tv_nsec = storage->tv_nsec;
		if (nsec_offset > tv_nsec) {
			storage->tv_sec -= 1;
			tv_nsec += nsecs_per_sec;
		}
		nsecs = tv_nsec - nsec_offset;
		secs = storage->tv_sec - sec_offset;
	}

	if (nsecs > nsecs_per_sec) {
		nsecs -= nsecs_per_sec;
		secs += 1;
	}
	storage->tv_nsec = (long)nsecs;
	storage->tv_sec = (time_t)secs;

	return storage;
}
