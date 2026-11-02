#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-nix-system-info.h"

/*
 * Characterization of the distribution, Flatpak and desktop log rules of
 * libobs/obs-nix.c and obs-nix-os-release.h, moved to
 * libobs/obs-nix-log-rules.c before their Rust port:
 * - obs_nix_system_info_os_release_field(), _os_release_value() and
 *   _version_fallback(): read_distribution_info()'s key checks, value span
 *   and quote cut, and the version when the file gave none;
 * - obs_nix_system_info_extension_name(): log_flatpak_extensions()'s name;
 * - obs_nix_system_info_desktop_form(): log_desktop_session_info()'s form.
 * Saved os-release text and extension lists run through a copy of each
 * caller's loop, so the lines they would log are pinned too. Cases marked
 * "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. It needs nothing from libobs, so it runs
 * on every platform. test_distribution_info runs read_distribution_info()
 * itself (Linux).
 */

/* ------------------------------------------------------------------------ */
/* os-release */

static void check_field(const char *line, const char *value, enum obs_nix_os_release_key key)
{
	enum obs_nix_os_release_key got = (enum obs_nix_os_release_key)99;
	const char *start = obs_nix_system_info_os_release_field(line, &got);
	if (!value) {
		assert_null(start);
		/* *key is left alone */
		assert_int_equal(got, 99);
		return;
	}
	assert_non_null(start);
	assert_ptr_equal(start, line + strlen(line) - strlen(value));
	assert_string_equal(start, value);
	assert_int_equal(got, key);
}

static void os_release_keys(void **state)
{
	(void)state;
	check_field("NAME=Arch\n", "Arch\n", OBS_NIX_OS_RELEASE_NAME);
	check_field("VERSION_ID=\"26.04\"\n", "\"26.04\"\n", OBS_NIX_OS_RELEASE_VERSION_ID);
	check_field("BUILD_ID=rolling\n", "rolling\n", OBS_NIX_OS_RELEASE_BUILD_ID);
	check_field("NAME=", "", OBS_NIX_OS_RELEASE_NAME);
	check_field("NAME==x", "=x", OBS_NIX_OS_RELEASE_NAME);

	/* compared by prefix and case, from the first byte */
	check_field("PRETTY_NAME=Arch\n", NULL, 0);
	check_field("NAME =x\n", NULL, 0);
	check_field(" NAME=x\n", NULL, 0);
	check_field("name=x\n", NULL, 0);
	check_field("NAME\n", NULL, 0);
	check_field("NAM", NULL, 0);
	check_field("VERSION_ID\n", NULL, 0);
	check_field("VERSION=1\n", NULL, 0);
	check_field("BUILD_IDX=1\n", NULL, 0);
	check_field("# NAME=x\n", NULL, 0);
	check_field("", NULL, 0);
}

static void check_value(const char *start, const char *expected)
{
	size_t len = 12345;
	const char *value = obs_nix_system_info_os_release_value(start, &len);
	assert_int_equal(len, strlen(expected));
	assert_true(value == start || value == start + 1);
	assert_memory_equal(value, expected, len);
}

static void os_release_values(void **state)
{
	(void)state;
	check_value("Arch\n", "Arch");
	check_value("\"Arch Linux\"\n", "Arch Linux");
	check_value("'x'\n", "x");
	check_value("\"26.04\"\r\n", "26.04");
	check_value("plain\r", "plain");
	check_value("last line", "last line");
	check_value("", "");
	check_value("\n", "");
	check_value("\"\"\n", "");
	check_value("''", "");

	/* one byte is not a pair: kept (characterized, not endorsed) */
	check_value("\"\n", "\"");
	check_value("'", "'");

	/* the quotes must match, and only one pair is cut: no escapes, no
	 * shell parse (characterized, not endorsed) */
	check_value("\"unterminated\n", "\"unterminated");
	check_value("'a\"\n", "'a\"");
	check_value("\"a'\"\n", "a'");
	check_value("\"\"a\"\"\n", "\"a\"");
	check_value("\"a\\\"\"\n", "a\\\"");
	check_value("\"$HOME\"\n", "$HOME");
	check_value("a\"b\"\n", "a\"b\"");
	check_value("\"a\" # x\n", "\"a\" # x");

	/* the span stops at the first CR or LF, and at a NUL */
	check_value("a\rb\n", "a");
	check_value("\"a\rb\"\n", "\"a");
	check_value("\"a\0b\"\n", "\"a");
}

static void version_fallback(void **state)
{
	(void)state;
	assert_null(obs_nix_system_info_version_fallback(true, "Arch Linux"));
	assert_null(obs_nix_system_info_version_fallback(true, "Unknown"));
	assert_string_equal(obs_nix_system_info_version_fallback(false, "Arch Linux"), "rolling");
	assert_string_equal(obs_nix_system_info_version_fallback(false, "Unknown"), "Unknown");
	assert_string_equal(obs_nix_system_info_version_fallback(false, "unknown"), "rolling");
	assert_string_equal(obs_nix_system_info_version_fallback(false, "Unknown "), "rolling");
	assert_string_equal(obs_nix_system_info_version_fallback(false, ""), "rolling");
}

/* read_distribution_info()'s loop over saved text, with fixed buffers for
 * its dstrs: a value is copied only when its length is not zero. */
struct distro_run {
	char distro[64];
	char version[64];
};

static void copy_value(char *dst, const char *start)
{
	size_t len;
	const char *value = obs_nix_system_info_os_release_value(start, &len);
	if (len) {
		assert_true(len < 64);
		memcpy(dst, value, len);
		dst[len] = '\0';
	}
}

static void run_distribution(const char *text, size_t size, struct distro_run *run)
{
	char build_id[64] = "";
	strcpy(run->distro, "Unknown");
	run->version[0] = '\0';

	const char *end = text + size;
	while (text < end) {
		/* the next line as getline() returns it: up to and including
		 * '\n', or the rest */
		const char *nl = memchr(text, '\n', (size_t)(end - text));
		size_t len = nl ? (size_t)(nl - text) + 1 : (size_t)(end - text);
		char line[128];
		assert_true(len < sizeof(line));
		memcpy(line, text, len);
		line[len] = '\0';
		text += len;

		enum obs_nix_os_release_key key;
		const char *start = obs_nix_system_info_os_release_field(line, &key);
		if (!start)
			continue;
		if (key == OBS_NIX_OS_RELEASE_NAME)
			copy_value(run->distro, start);
		else if (key == OBS_NIX_OS_RELEASE_VERSION_ID)
			copy_value(run->version, start);
		else
			copy_value(build_id, start);
	}
	if (!run->version[0]) {
		const char *fallback = obs_nix_system_info_version_fallback(build_id[0] != '\0', run->distro);
		strcpy(run->version, fallback ? fallback : build_id);
	}
}

static void check_distribution_n(const char *text, size_t size, const char *distro, const char *version)
{
	struct distro_run run;
	run_distribution(text, size, &run);
	assert_string_equal(run.distro, distro);
	assert_string_equal(run.version, version);
}

static void check_distribution(const char *text, const char *distro, const char *version)
{
	check_distribution_n(text, strlen(text), distro, version);
}

static void distribution_files(void **state)
{
	(void)state;
	check_distribution("NAME=\"Arch Linux\"\nBUILD_ID=rolling\n", "Arch Linux", "rolling");
	check_distribution("NAME='x'\n", "x", "rolling");
	check_distribution("NAME=\"unterminated\n", "\"unterminated", "rolling");
	check_distribution("NAME=Ubuntu\r\nVERSION_ID=\"26.04\"\r\n", "Ubuntu", "26.04");
	check_distribution("NAME=Fedora\nVERSION_ID=43", "Fedora", "43");

	/* an empty value is not copied */
	check_distribution("NAME=\n", "Unknown", "Unknown");
	check_distribution("NAME=\"\"\n", "Unknown", "Unknown");
	/* one quote is a value (characterized, not endorsed) */
	check_distribution("NAME=\"\n", "\"", "rolling");

	/* keys that don't match */
	check_distribution("PRETTY_NAME=\"Arch Linux\"\nNAME =x\n", "Unknown", "Unknown");

	/* a later key replaces an earlier one, unless it is empty */
	check_distribution("NAME=a\nVERSION_ID=1\nVERSION_ID=2\nVERSION_ID=\n", "a", "2");
	check_distribution("NAME=a\nNAME=b\nNAME=''\n", "b", "rolling");

	/* the fallback */
	check_distribution("NAME=Arch\nBUILD_ID=20261013\n", "Arch", "20261013");
	check_distribution("BUILD_ID=b\n", "Unknown", "b");
	check_distribution("NAME=Arch\n", "Arch", "rolling");
	check_distribution("", "Unknown", "Unknown");
	/* NAME=Unknown in the file reads as no name (characterized, not
	 * endorsed) */
	check_distribution("NAME=Unknown\n", "Unknown", "Unknown");

	/* a NUL inside a line ends it for every check */
	check_distribution_n("NAME=a\0b\nVERSION_ID=\"1\0\"\n", 25, "a", "\"1");
	check_distribution_n("\0NAME=a\n", 8, "Unknown", "Unknown");
}

/* ------------------------------------------------------------------------ */
/* Flatpak extensions */

static void check_name(const char *ext, const char *expected)
{
	size_t len = 12345;
	const char *name = obs_nix_system_info_extension_name(ext, &len);
	assert_non_null(name);
	assert_int_equal(len, strlen(expected));
	assert_memory_equal(name, expected, len);
	if (strcmp(expected, "(null)") != 0)
		assert_true(name >= ext && name + len <= ext + strlen(ext));
}

static void extension_names(void **state)
{
	(void)state;
	check_name("org.freedesktop.Platform.GL.default=4c1f8b2a", "org.freedesktop.Platform.GL.default");
	check_name("a=1", "a");
	check_name("a", "a");
	check_name("a=", "a");
	check_name("a==b", "a");
	check_name("a=b=c", "a");

	/* an empty name logs the next non-empty piece, the commit hash
	 * (characterized, not endorsed) */
	check_name("=abc", "abc");
	check_name("==abc=d", "abc");

	/* no piece at all: blog()'s "(null)" for the NULL entry
	 * (characterized, not endorsed) */
	check_name("=", "(null)");
	check_name("==", "(null)");
	check_name("", "(null)");
}

/* log_flatpak_extensions()'s loop: strlist_split(extensions, ';', false),
 * which drops empty pieces, then one line per entry. */
static size_t run_extensions(const char *extensions, char lines[][64], size_t max)
{
	size_t count = 0;
	const char *cur = extensions;
	for (;;) {
		size_t piece = strcspn(cur, ";");
		if (piece) {
			char ext[64];
			assert_true(piece < sizeof(ext));
			memcpy(ext, cur, piece);
			ext[piece] = '\0';

			size_t len;
			const char *name = obs_nix_system_info_extension_name(ext, &len);
			if (name) {
				assert_true(count < max);
				snprintf(lines[count++], 64, " - %.*s", (int)len, name);
			}
		}
		if (cur[piece] == '\0')
			break;
		cur += piece + 1;
	}
	return count;
}

static void check_extensions(const char *extensions, size_t count, const char *const *expected)
{
	char lines[8][64];
	size_t got = run_extensions(extensions, lines, 8);
	assert_int_equal(got, count);
	for (size_t i = 0; i < count; i++)
		assert_string_equal(lines[i], expected[i]);
}

static void extension_lists(void **state)
{
	(void)state;
	const char *ab[] = {" - a", " - b"};
	check_extensions("a=1;b=2", 2, ab);
	check_extensions("a;b", 2, ab);
	check_extensions("a=1;;b=2", 2, ab);
	check_extensions(";a=1;b=2;", 2, ab);

	const char *a[] = {" - a"};
	check_extensions("a=1;", 1, a);

	const char *hash[] = {" - a", " - abc"};
	check_extensions("a=1;=abc", 2, hash);
	const char *null[] = {" - (null)", " - b"};
	check_extensions("=;b=2", 2, null);

	/* an empty value logs the heading and no entries (characterized) */
	check_extensions("", 0, NULL);
	check_extensions(";;", 0, NULL);
}

/* ------------------------------------------------------------------------ */
/* desktop */

static void desktop_forms(void **state)
{
	(void)state;
	assert_int_equal(obs_nix_system_info_desktop_form(true, true), OBS_NIX_DESKTOP_BOTH);
	assert_int_equal(obs_nix_system_info_desktop_form(true, false), OBS_NIX_DESKTOP_ONE);
	assert_int_equal(obs_nix_system_info_desktop_form(false, true), OBS_NIX_DESKTOP_ONE);
	assert_int_equal(obs_nix_system_info_desktop_form(false, false), OBS_NIX_DESKTOP_NONE);
	/* the constants the callers switch on */
	assert_int_equal(OBS_NIX_DESKTOP_NONE, 0);
	assert_int_equal(OBS_NIX_DESKTOP_ONE, 1);
	assert_int_equal(OBS_NIX_DESKTOP_BOTH, 2);
	assert_int_equal(OBS_NIX_OS_RELEASE_NAME, 0);
	assert_int_equal(OBS_NIX_OS_RELEASE_VERSION_ID, 1);
	assert_int_equal(OBS_NIX_OS_RELEASE_BUILD_ID, 2);
}

/* log_desktop_session_info()'s line, from the form. */
static void desktop_line(const char *current, const char *session, char *line, size_t size)
{
	line[0] = '\0';
	switch (obs_nix_system_info_desktop_form(current != NULL, session != NULL)) {
	case OBS_NIX_DESKTOP_BOTH:
		snprintf(line, size, "Desktop Environment: %s (%s)", current, session);
		break;
	case OBS_NIX_DESKTOP_ONE:
		snprintf(line, size, "Desktop Environment: %s", current ? current : session);
		break;
	case OBS_NIX_DESKTOP_NONE:
		break;
	}
}

static void check_desktop(const char *current, const char *session, const char *expected)
{
	char line[64];
	desktop_line(current, session, line, sizeof(line));
	assert_string_equal(line, expected);
}

static void desktop_lines(void **state)
{
	(void)state;
	check_desktop("GNOME", "gnome", "Desktop Environment: GNOME (gnome)");
	check_desktop("KDE", NULL, "Desktop Environment: KDE");
	check_desktop(NULL, "plasma", "Desktop Environment: plasma");
	check_desktop(NULL, NULL, "");

	/* an empty value counts as set (characterized, not endorsed) */
	check_desktop("", "", "Desktop Environment:  ()");
	check_desktop("", NULL, "Desktop Environment: ");
	check_desktop(NULL, "", "Desktop Environment: ");
	check_desktop("", "gnome", "Desktop Environment:  (gnome)");
	check_desktop("GNOME", "", "Desktop Environment: GNOME ()");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(os_release_keys),  cmocka_unit_test(os_release_values),
		cmocka_unit_test(version_fallback), cmocka_unit_test(distribution_files),
		cmocka_unit_test(extension_names),  cmocka_unit_test(extension_lists),
		cmocka_unit_test(desktop_forms),    cmocka_unit_test(desktop_lines),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
