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

/* os_inhibit_sleep_* for Linux and FreeBSD, moved out of platform-nix.c so
 * it is compiled in both builds: ENABLE_RUST_LIBOBS replaces platform-nix.c
 * with the Rust port, but sleep inhibition stays C because it needs GIO for
 * the portal (platform-nix-portal.c) and session D-Bus (platform-nix-dbus.c)
 * inhibitors. macOS uses platform-cocoa.m and Windows platform-windows.c. */

#include <errno.h>
#include <signal.h>
#include <spawn.h>
#include <stdbool.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

#include "obsconfig.h"

#include "base.h"
#include "bmem.h"
#include "platform.h"
#include "threading.h"

#if defined(GIO_FOUND)
struct dbus_sleep_info;
struct portal_inhibit_info;

extern struct dbus_sleep_info *dbus_sleep_info_create(void);
extern void dbus_inhibit_sleep(struct dbus_sleep_info *dbus, const char *sleep, bool active);
extern void dbus_sleep_info_destroy(struct dbus_sleep_info *dbus);

extern struct portal_inhibit_info *portal_inhibit_info_create(void);
extern void portal_inhibit(struct portal_inhibit_info *portal, const char *reason, bool active);
extern void portal_inhibit_info_destroy(struct portal_inhibit_info *portal);
#endif

struct os_inhibit_info {
#if defined(GIO_FOUND)
	struct dbus_sleep_info *dbus;
	struct portal_inhibit_info *portal;
#endif
	pthread_t screensaver_thread;
	os_event_t *stop_event;
	char *reason;
	posix_spawnattr_t attr;
	bool active;
};

os_inhibit_t *os_inhibit_sleep_create(const char *reason)
{
	struct os_inhibit_info *info = bzalloc(sizeof(*info));
	sigset_t set;

#if defined(GIO_FOUND)
	info->portal = portal_inhibit_info_create();
	if (!info->portal) {
		/* In a Flatpak, only the portal can be used for inhibition. */
		if (access("/.flatpak-info", F_OK) == 0) {
			bfree(info);
			return NULL;
		}

		info->dbus = dbus_sleep_info_create();
	}

	if (info->portal || info->dbus) {
		info->reason = bstrdup(reason);
		return info;
	}
#endif
	os_event_init(&info->stop_event, OS_EVENT_TYPE_AUTO);
	posix_spawnattr_init(&info->attr);

	sigemptyset(&set);
	posix_spawnattr_setsigmask(&info->attr, &set);
	sigaddset(&set, SIGPIPE);
	posix_spawnattr_setsigdefault(&info->attr, &set);
	posix_spawnattr_setflags(&info->attr, POSIX_SPAWN_SETSIGDEF | POSIX_SPAWN_SETSIGMASK);

	info->reason = bstrdup(reason);
	return info;
}

extern char **environ;

static void reset_screensaver(os_inhibit_t *info)
{
	char *argv[3] = {(char *)"xdg-screensaver", (char *)"reset", NULL};
	pid_t pid;

	int err = posix_spawnp(&pid, "xdg-screensaver", NULL, &info->attr, argv, environ);
	if (err == 0) {
		int status;
		while (waitpid(pid, &status, 0) == -1)
			;
	} else {
		blog(LOG_WARNING, "Failed to create xdg-screensaver: %d", err);
	}
}

static void *screensaver_thread(void *param)
{
	os_inhibit_t *info = param;

	while (os_event_timedwait(info->stop_event, 30000) == ETIMEDOUT)
		reset_screensaver(info);

	return NULL;
}

bool os_inhibit_sleep_set_active(os_inhibit_t *info, bool active)
{
	int ret;

	if (!info)
		return false;
	if (info->active == active)
		return false;

#if defined(GIO_FOUND)
	if (info->portal)
		portal_inhibit(info->portal, info->reason, active);
	if (info->dbus)
		dbus_inhibit_sleep(info->dbus, info->reason, active);
	if (info->portal || info->dbus) {
		info->active = active;
		return true;
	}
#endif

	if (!info->stop_event)
		return true;

	if (active) {
		ret = pthread_create(&info->screensaver_thread, NULL, &screensaver_thread, info);
		if (ret < 0) {
			blog(LOG_ERROR, "Failed to create screensaver "
					"inhibitor thread");
			return false;
		}
	} else {
		os_event_signal(info->stop_event);
		pthread_join(info->screensaver_thread, NULL);
	}

	info->active = active;
	return true;
}

void os_inhibit_sleep_destroy(os_inhibit_t *info)
{
	if (info) {
		os_inhibit_sleep_set_active(info, false);
#if defined(GIO_FOUND)
		if (info->portal) {
			portal_inhibit_info_destroy(info->portal);
		} else if (info->dbus) {
			dbus_sleep_info_destroy(info->dbus);
		} else {
			os_event_destroy(info->stop_event);
			posix_spawnattr_destroy(&info->attr);
		}
#else
		os_event_destroy(info->stop_event);
		posix_spawnattr_destroy(&info->attr);
#endif

		bfree(info->reason);
		bfree(info);
	}
}
