/* Test-only stand-ins for what libobs/util/platform-nix-inhibit.c calls
 * outside itself (see nix_inhibit_hooks.h): the portal and session D-Bus
 * inhibitors, the Flatpak check, the xdg-screensaver spawn, blog and the reset
 * thread's wait. Each call is appended to one log, so the C oracle and the
 * Rust core, whose test host calls these same functions, can be compared
 * line for line. Linux and FreeBSD only.
 *
 * Nothing here is thread-safe across tests: the parity tests hold one lock. */
#if defined(__linux__) || defined(__FreeBSD__)

#include <errno.h>
#include <pthread.h>
#include <spawn.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include <time.h>

#include "util/threading.h"

struct portal_inhibit_info;
struct dbus_sleep_info;

/* What the original file and the Rust test host call. */
struct portal_inhibit_info *portal_inhibit_info_create(void);
void portal_inhibit(struct portal_inhibit_info *portal, const char *reason, bool active);
void portal_inhibit_info_destroy(struct portal_inhibit_info *portal);
struct dbus_sleep_info *dbus_sleep_info_create(void);
void dbus_inhibit_sleep(struct dbus_sleep_info *dbus, const char *sleep, bool active);
void dbus_sleep_info_destroy(struct dbus_sleep_info *dbus);
int oracle_stub_inhibit_access(const char *path, int mode);
int oracle_stub_inhibit_posix_spawnp(pid_t *pid, const char *file, const posix_spawn_file_actions_t *actions,
				     const posix_spawnattr_t *attr, char *const argv[], char *const envp[]);
void oracle_stub_inhibit_blog(int level, const char *format, ...) __attribute__((format(printf, 2, 3)));
int oracle_stub_inhibit_timedwait(os_event_t *event, unsigned long milliseconds);

/* Controls for the tests, and the Rust test host's way into the log. */
void oracle_inhibit_stub_reset(int portal_ok, int dbus_ok, int flatpak, int spawn_error);
void oracle_inhibit_stub_set_timeouts(int timeouts);
size_t oracle_inhibit_stub_take_log(char *out, size_t cap);
int oracle_inhibit_stub_await_parked(int threads);
int oracle_inhibit_stub_wait_enter(unsigned long milliseconds);
void oracle_inhibit_stub_wait_leave(void);
int oracle_inhibit_stub_spawn(const char *file, const char *argv0, const char *argv1);
void oracle_inhibit_stub_log_line(int level, const char *message);

static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t parked_cond = PTHREAD_COND_INITIALIZER;
static char log_text[1 << 16];
static size_t log_len;
static bool portal_ok;
static bool dbus_ok;
static bool flatpak;
static int spawn_error = ENOENT;
static int timeouts;
static int parked;
/* Handles are addresses in here, numbered from 1 in creation order. */
static char handles[64];
static int next_handle;

static void vappend(const char *format, va_list args) __attribute__((format(printf, 1, 0)));
static void vappend(const char *format, va_list args)
{
	if (log_len >= sizeof(log_text) - 1)
		return;
	int n = vsnprintf(log_text + log_len, sizeof(log_text) - log_len, format, args);
	if (n < 0)
		return;
	log_len += (size_t)n;
	if (log_len > sizeof(log_text) - 2)
		log_len = sizeof(log_text) - 2;
	log_text[log_len++] = '\n';
	log_text[log_len] = '\0';
}

/* Appends one line. The caller holds `lock`. */
static void append_locked(const char *format, ...) __attribute__((format(printf, 1, 2)));
static void append_locked(const char *format, ...)
{
	va_list args;
	va_start(args, format);
	vappend(format, args);
	va_end(args);
}

static void append(const char *format, ...) __attribute__((format(printf, 1, 2)));
static void append(const char *format, ...)
{
	va_list args;
	pthread_mutex_lock(&lock);
	va_start(args, format);
	vappend(format, args);
	va_end(args);
	pthread_mutex_unlock(&lock);
}

static void *new_handle(bool ok)
{
	if (!ok)
		return NULL;
	pthread_mutex_lock(&lock);
	int index = next_handle++ % (int)sizeof(handles);
	pthread_mutex_unlock(&lock);
	return &handles[index];
}

static long handle_number(const void *handle)
{
	return handle ? (const char *)handle - handles + 1 : 0;
}

static const char *text(const char *reason)
{
	return reason ? reason : "(null)";
}

struct portal_inhibit_info *portal_inhibit_info_create(void)
{
	void *handle = new_handle(portal_ok);
	append("portal_create %ld", handle_number(handle));
	return handle;
}

void portal_inhibit(struct portal_inhibit_info *portal, const char *reason, bool active)
{
	append("portal_inhibit %ld [%s] %d", handle_number(portal), text(reason), active);
}

void portal_inhibit_info_destroy(struct portal_inhibit_info *portal)
{
	append("portal_destroy %ld", handle_number(portal));
}

struct dbus_sleep_info *dbus_sleep_info_create(void)
{
	void *handle = new_handle(dbus_ok);
	append("dbus_create %ld", handle_number(handle));
	return handle;
}

void dbus_inhibit_sleep(struct dbus_sleep_info *dbus, const char *sleep, bool active)
{
	append("dbus_inhibit %ld [%s] %d", handle_number(dbus), text(sleep), active);
}

void dbus_sleep_info_destroy(struct dbus_sleep_info *dbus)
{
	append("dbus_destroy %ld", handle_number(dbus));
}

int oracle_stub_inhibit_access(const char *path, int mode)
{
	append("access %s %d", path, mode);
	if (flatpak && strcmp(path, "/.flatpak-info") == 0)
		return 0;
	errno = ENOENT;
	return -1;
}

int oracle_inhibit_stub_spawn(const char *file, const char *argv0, const char *argv1)
{
	append("spawn %s %s %s", file, text(argv0), text(argv1));
	return spawn_error;
}

/* Never starts anything: a 0 would send the C to waitpid on no child. */
int oracle_stub_inhibit_posix_spawnp(pid_t *pid, const char *file, const posix_spawn_file_actions_t *actions,
				     const posix_spawnattr_t *attr, char *const argv[], char *const envp[])
{
	(void)pid;
	(void)actions;
	(void)attr;
	(void)envp;
	return oracle_inhibit_stub_spawn(file, argv[0], argv[0] ? argv[1] : NULL);
}

void oracle_inhibit_stub_log_line(int level, const char *message)
{
	append("log %d %s", level, message);
}

void oracle_stub_inhibit_blog(int level, const char *format, ...)
{
	char message[1024];
	va_list args;
	va_start(args, format);
	vsnprintf(message, sizeof(message), format, args);
	va_end(args);
	oracle_inhibit_stub_log_line(level, message);
}

int oracle_inhibit_stub_wait_enter(unsigned long milliseconds)
{
	pthread_mutex_lock(&lock);
	if (timeouts > 0) {
		timeouts--;
		append_locked("wait %lu timeout", milliseconds);
		pthread_mutex_unlock(&lock);
		return 1;
	}
	append_locked("wait %lu", milliseconds);
	parked++;
	pthread_cond_broadcast(&parked_cond);
	pthread_mutex_unlock(&lock);
	return 0;
}

void oracle_inhibit_stub_wait_leave(void)
{
	pthread_mutex_lock(&lock);
	parked--;
	pthread_cond_broadcast(&parked_cond);
	pthread_mutex_unlock(&lock);
}

int oracle_stub_inhibit_timedwait(os_event_t *event, unsigned long milliseconds)
{
	if (oracle_inhibit_stub_wait_enter(milliseconds))
		return ETIMEDOUT;
	int ret = os_event_timedwait(event, milliseconds);
	oracle_inhibit_stub_wait_leave();
	return ret;
}

void oracle_inhibit_stub_reset(int portal, int dbus, int in_flatpak, int spawn_errno)
{
	pthread_mutex_lock(&lock);
	portal_ok = portal != 0;
	dbus_ok = dbus != 0;
	flatpak = in_flatpak != 0;
	spawn_error = spawn_errno ? spawn_errno : ENOENT;
	timeouts = 0;
	next_handle = 0;
	log_len = 0;
	log_text[0] = '\0';
	pthread_mutex_unlock(&lock);
}

void oracle_inhibit_stub_set_timeouts(int count)
{
	pthread_mutex_lock(&lock);
	timeouts = count;
	pthread_mutex_unlock(&lock);
}

size_t oracle_inhibit_stub_take_log(char *out, size_t cap)
{
	pthread_mutex_lock(&lock);
	size_t len = log_len;
	if (out && cap) {
		size_t n = len < cap - 1 ? len : cap - 1;
		memcpy(out, log_text, n);
		out[n] = '\0';
	}
	log_len = 0;
	log_text[0] = '\0';
	pthread_mutex_unlock(&lock);
	return len;
}

/* Blocks until `threads` reset threads are parked in their real wait, so
 * the next step cannot race their log lines. 0 after 10 s. */
int oracle_inhibit_stub_await_parked(int threads)
{
	struct timespec deadline;
	clock_gettime(CLOCK_REALTIME, &deadline);
	deadline.tv_sec += 10;

	int ok = 1;
	pthread_mutex_lock(&lock);
	while (parked != threads) {
		if (pthread_cond_timedwait(&parked_cond, &lock, &deadline) == ETIMEDOUT) {
			ok = parked == threads;
			break;
		}
	}
	pthread_mutex_unlock(&lock);
	return ok;
}

#else
typedef int oracle_nix_inhibit_stubs_unused;
#endif
