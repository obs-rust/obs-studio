#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <setjmp.h>
#include <cmocka.h>

#include <obs.h>
#include <util/base.h>
#include "obs-nix-os-release.h"

/*
 * The distribution and desktop lines of the Linux startup log, through the
 * real libobs: obs_startup() runs log_system_info(), whose
 * read_distribution_info() and log_desktop_session_info() call the
 * obs_nix_system_info_* log rules. In the hybrid libobs those are the Rust
 * shims (rust/obs-core, nix_system_info), so this checks them through the
 * callers they replace; against libobs-c-reference it checks the C.
 *
 * - every set, unset and empty pair of XDG_CURRENT_DESKTOP and
 *   XDG_SESSION_DESKTOP, and XDG_SESSION_TYPE set and unset: the exact
 *   desktop and session lines (an empty name counts as set);
 * - the distribution line, against read_distribution_info() compiled from
 *   the C rules into this test over the same /etc/os-release, or the
 *   missing-file line. Inside a Flatpak, where libobs logs the Flatpak
 *   lines instead, the case checks only that it logs no distribution line.
 */

#define MAX_LINES 256

static char *lines[MAX_LINES];
static size_t line_count;

static void capture(int level, const char *format, va_list args, void *param)
{
	(void)level;
	(void)param;
	char buf[1024];
	vsnprintf(buf, sizeof(buf), format, args);
	if (line_count < MAX_LINES) {
		lines[line_count] = malloc(strlen(buf) + 1);
		strcpy(lines[line_count], buf);
		line_count++;
	}
}

static void clear_lines(void)
{
	for (size_t i = 0; i < line_count; i++)
		free(lines[i]);
	line_count = 0;
}

/* The one logged line that starts with prefix, or NULL; fails on two. */
static const char *find_line(const char *prefix)
{
	const char *found = NULL;
	for (size_t i = 0; i < line_count; i++) {
		if (strncmp(lines[i], prefix, strlen(prefix)) == 0) {
			assert_null(found);
			found = lines[i];
		}
	}
	return found;
}

/* The startup log, captured. log_system_info() runs before anything that
 * needs a display, and a failed obs_startup() shuts down on its own, so the
 * lines are there either way. */
static void start_and_stop(void)
{
	clear_lines();
	base_set_log_handler(capture, NULL);
	if (obs_startup("en-US", NULL, NULL))
		obs_shutdown();
	base_set_log_handler(NULL, NULL);
	assert_non_null(find_line("Kernel Version:"));
}

static void set_env(const char *name, const char *value)
{
	if (value)
		setenv(name, value, 1);
	else
		unsetenv(name);
}

static void check_desktop(const char *current, const char *session, const char *expected)
{
	set_env("XDG_CURRENT_DESKTOP", current);
	set_env("XDG_SESSION_DESKTOP", session);
	start_and_stop();
	const char *line = find_line("Desktop Environment:");
	if (!expected) {
		assert_null(line);
		return;
	}
	assert_non_null(line);
	assert_string_equal(line, expected);
}

static void desktop_lines(void **state)
{
	(void)state;
	check_desktop("GNOME", "gnome", "Desktop Environment: GNOME (gnome)");
	check_desktop("KDE", NULL, "Desktop Environment: KDE");
	check_desktop(NULL, "plasma", "Desktop Environment: plasma");
	check_desktop(NULL, NULL, NULL);

	/* an empty name counts as set (characterized, not endorsed) */
	check_desktop("", "", "Desktop Environment:  ()");
	check_desktop("", NULL, "Desktop Environment: ");
	check_desktop(NULL, "", "Desktop Environment: ");
	check_desktop("GNOME", "", "Desktop Environment: GNOME ()");
}

static void session_type_line(void **state)
{
	(void)state;
	set_env("XDG_SESSION_TYPE", "wayland");
	start_and_stop();
	const char *line = find_line("Session Type:");
	assert_non_null(line);
	assert_string_equal(line, "Session Type: wayland");

	set_env("XDG_SESSION_TYPE", NULL);
	start_and_stop();
	assert_null(find_line("Session Type:"));
}

static void distribution_line(void **state)
{
	(void)state;
	start_and_stop();
	const char *line = find_line("Distribution:");

	if (access("/.flatpak-info", F_OK) == 0) {
		assert_null(line);
		assert_non_null(find_line("Flatpak Branch:"));
		return;
	}

	assert_non_null(line);
	FILE *fp = fopen("/etc/os-release", "r");
	if (!fp) {
		assert_string_equal(line, "Distribution: Missing /etc/os-release !");
		return;
	}

	struct dstr distro, version, expected;
	read_distribution_info(fp, &distro, &version);
	fclose(fp);
	dstr_init(&expected);
	dstr_printf(&expected, "Distribution: %s %s", distro.array, version.array);
	assert_string_equal(line, expected.array);
	dstr_free(&expected);
	dstr_free(&version);
	dstr_free(&distro);
}

static int teardown(void **state)
{
	(void)state;
	clear_lines();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(desktop_lines),
		cmocka_unit_test(session_type_line),
		cmocka_unit_test(distribution_line),
	};
	return cmocka_run_group_tests(tests, NULL, teardown);
}
