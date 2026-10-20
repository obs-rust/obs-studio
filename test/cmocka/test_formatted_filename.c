/*
 * Characterization of os_generate_formatted_filename (libobs/util/
 * platform-filename.c) before its port (#262 slice 5): every specifier, unknown
 * specifiers, spaces, the extension, an empty format and the video info it
 * reads, plus the 255-byte UTF-8 cut (obs-rust/obs-studio#21). Cases marked
 * "characterized, not endorsed" pin odd current behavior.
 *
 * platform-filename.c is compiled into this test (see CMakeLists.txt), so it
 * characterizes the C source only: it cannot exercise the Rust swap.
 *
 * Nothing depends on the clock, the time zone or the locale: time() below is a
 * fixed clock, the group setup sets a fixed TZ and the C locale.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdlib.h>
#include <string.h>
#include <locale.h>
#include <time.h>
#include <cmocka.h>

#include <obs.h>
#include <util/bmem.h>
#include <util/dstr.h>
#include <util/platform.h>

/* The fixed clock: 2025-11-07 10:39:03 UTC, a Friday. In TEST_TZ (UTC+05:30,
 * abbreviation OBS) that is 16:09:03 local, so every field differs from the
 * others and from UTC. This definition replaces libc's time() for
 * platform-filename.c, which is compiled into the test. */
#define FIXED_NOW ((time_t)1762511943)
#define TEST_TZ "OBS-5:30"

time_t time(time_t *t)
{
	if (t)
		*t = FIXED_NOW;
	return FIXED_NOW;
}

/* platform-filename.c is compiled into this test (see CMakeLists.txt), so this
 * fixture replaces libobs's obs_get_video_info and no obs core needs to be
 * started. reset_video sets 30/1 fps with every other field zero. */
static struct obs_video_info fixture_ovi;
static bool fixture_result;
static int video_info_calls;

bool obs_get_video_info(struct obs_video_info *ovi)
{
	video_info_calls++;
	*ovi = fixture_ovi;
	return fixture_result;
}

static int reset_video(void **state)
{
	UNUSED_PARAMETER(state);

	memset(&fixture_ovi, 0, sizeof(fixture_ovi));
	fixture_ovi.fps_num = 30;
	fixture_ovi.fps_den = 1;
	fixture_result = true;
	video_info_calls = 0;
	return 0;
}

static int fixed_tz_and_locale(void **state)
{
	UNUSED_PARAMETER(state);

	setenv("TZ", TEST_TZ, 1);
	tzset();
	setlocale(LC_ALL, "C");
	return 0;
}

/* Expects os_generate_formatted_filename(extension, space, format) to return
 * expected, or NULL when expected is NULL. */
static void expect_name(const char *extension, bool space, const char *format, const char *expected)
{
	char *name = os_generate_formatted_filename(extension, space, format);

	printf("format [%s] extension [%s] space %d -> [%s]\n", format ? format : "(null)",
	       extension ? extension : "(null)", space, name ? name : "(null)");
	if (expected)
		assert_string_equal(name, expected);
	else
		assert_null(name);
	bfree(name);
}

struct name_case {
	const char *format;
	const char *expected;
};

static void expect_names(const struct name_case *cases, size_t count)
{
	for (size_t i = 0; i < count; i++)
		expect_name(NULL, true, cases[i].format, cases[i].expected);
}

/* Strict UTF-8 check: rejects truncated, overlong and surrogate sequences. */
static bool is_valid_utf8(const unsigned char *s, size_t len)
{
	size_t i = 0;
	while (i < len) {
		unsigned char c = s[i];
		size_t n;
		unsigned int cp;

		if (c < 0x80) {
			i++;
			continue;
		} else if ((c & 0xE0) == 0xC0) {
			n = 1;
			cp = c & 0x1F;
		} else if ((c & 0xF0) == 0xE0) {
			n = 2;
			cp = c & 0x0F;
		} else if ((c & 0xF8) == 0xF0) {
			n = 3;
			cp = c & 0x07;
		} else {
			return false;
		}

		if (i + n >= len)
			return false;
		for (size_t k = 1; k <= n; k++) {
			if ((s[i + k] & 0xC0) != 0x80)
				return false;
			cp = (cp << 6) | (s[i + k] & 0x3F);
		}

		if ((n == 1 && cp < 0x80) || (n == 2 && cp < 0x800) || (n == 3 && cp < 0x10000) || cp > 0x10FFFF ||
		    (cp >= 0xD800 && cp <= 0xDFFF))
			return false;
		i += n + 1;
	}
	return true;
}

/* The upstream report (obsproject/obs-studio#11211): a 23-byte prefix
 * followed by U+200B (3 bytes) puts the 255-byte cut inside a character. */
static void zero_width_space_report_test(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr fmt = {0};
	dstr_copy(&fmt, "%CCYY-%MM-%DD %hh-%mm-%ss foo");
	for (int i = 0; i < 100; i++)
		dstr_cat(&fmt, "\xE2\x80\x8B");
	dstr_cat(&fmt, "bar");

	char *name = os_generate_formatted_filename("mkv", true, fmt.array);
	size_t len = strlen(name);

	/* "YYYY-MM-DD hh-mm-ss foo" is 23 bytes; 77 full U+200B fit in 254. */
	assert_int_equal(len, 254);
	assert_true(is_valid_utf8((const unsigned char *)name, len));

	bfree(name);
	dstr_free(&fmt);
}

/* Every interior cut of a 2-, 3- and 4-byte character at the 255-byte limit
 * must drop the whole character; a character ending exactly at byte 255 must
 * be kept. */
static void every_interior_cut_test(void **state)
{
	UNUSED_PARAMETER(state);

	static const char *chars[] = {"\xC3\xA9", "\xE2\x80\x8B", "\xF0\x9F\x98\x80"};

	for (size_t c = 0; c < sizeof(chars) / sizeof(chars[0]); c++) {
		size_t clen = strlen(chars[c]);

		/* offset = bytes of the character that fit before the limit */
		for (size_t fit = 0; fit <= clen; fit++) {
			size_t prefix = 255 - fit;
			struct dstr fmt = {0};
			for (size_t i = 0; i < prefix; i++)
				dstr_cat_ch(&fmt, 'a');
			dstr_cat(&fmt, chars[c]);
			dstr_cat(&fmt, "tail");

			char *name = os_generate_formatted_filename(NULL, true, fmt.array);
			size_t len = strlen(name);
			size_t expected = fit == clen ? 255 : prefix;

			printf("char %zu-byte, %zu byte(s) before limit: len %zu\n", clen, fit, len);
			assert_int_equal(len, expected);
			assert_true(is_valid_utf8((const unsigned char *)name, len));

			bfree(name);
			dstr_free(&fmt);
		}
	}
}

static void ascii_truncation_unchanged_test(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr fmt = {0};
	for (int i = 0; i < 300; i++)
		dstr_cat_ch(&fmt, 'x');

	char *name = os_generate_formatted_filename("mp4", false, fmt.array);
	assert_int_equal(strlen(name), 255);
	assert_memory_equal(name, fmt.array, 255);

	bfree(name);
	dstr_free(&fmt);
}

/* Invalid input made only of continuation bytes must not back up past one
 * character's worth of bytes (or to an empty, NULL result). */
static void invalid_continuation_bytes_test(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr fmt = {0};
	dstr_cat_ch(&fmt, 'a');
	for (int i = 0; i < 300; i++)
		dstr_cat_ch(&fmt, (char)0x80);

	char *name = os_generate_formatted_filename(NULL, true, fmt.array);
	assert_non_null(name);
	assert_int_equal(strlen(name), 252);

	bfree(name);
	dstr_free(&fmt);
}

static void short_name_unchanged_test(void **state)
{
	UNUSED_PARAMETER(state);

	char *name = os_generate_formatted_filename("mkv", false, "caf\xC3\xA9 %FPS");
	assert_string_equal(name, "caf\xC3\xA9_30.mkv");
	bfree(name);
}

/* Every date and time specifier on its own. The OBS ones (%CCYY ... %ss) map
 * to strftime conversions; the strftime-named ones are passed to strftime as
 * they are. All of them use local time. */
static void date_time_specifiers_test(void **state)
{
	UNUSED_PARAMETER(state);

	static const struct name_case cases[] = {
		{"%CCYY", "2025"}, {"%YY", "25"},  {"%MM", "11"},    {"%DD", "07"}, {"%hh", "16"},      {"%mm", "09"},
		{"%ss", "03"},     {"%a", "Fri"},  {"%A", "Friday"}, {"%b", "Nov"}, {"%B", "November"}, {"%d", "07"},
		{"%H", "16"},      {"%I", "04"},   {"%m", "11"},     {"%M", "09"},  {"%p", "PM"},       {"%S", "03"},
		{"%y", "25"},      {"%Y", "2025"}, {"%z", "+0530"},  {"%Z", "OBS"},
	};

	expect_names(cases, sizeof(cases) / sizeof(cases[0]));
}

/* %s is not in the specifier table: it is the Unix time, in seconds. */
static void unix_time_specifier_test(void **state)
{
	UNUSED_PARAMETER(state);

	static const struct name_case cases[] = {
		{"%s", "1762511943"},
		{"t%s.", "t1762511943."},
	};

	expect_names(cases, sizeof(cases) / sizeof(cases[0]));
}

static void video_specifiers_test(void **state)
{
	UNUSED_PARAMETER(state);

	fixture_ovi.fps_num = 60;
	fixture_ovi.fps_den = 1;
	fixture_ovi.base_width = 1920;
	fixture_ovi.base_height = 1080;
	fixture_ovi.output_width = 1280;
	fixture_ovi.output_height = 720;
	fixture_ovi.output_format = VIDEO_FORMAT_NV12;

	static const struct name_case cases[] = {
		{"%FPS", "60"},
		{"%CRES", "1920x1080"},
		{"%ORES", "1280x720"},
		{"%VF", "NV12"},
		{"%CRES %ORES %FPS %VF", "1920x1080 1280x720 60 NV12"},
	};

	expect_names(cases, sizeof(cases) / sizeof(cases[0]));
}

/* %FPS is the integer fps_num when fps_den is 0 or 1, otherwise the quotient
 * with two decimals. */
static void fps_formatting_test(void **state)
{
	UNUSED_PARAMETER(state);

	static const struct {
		uint32_t num, den;
		const char *expected;
	} cases[] = {
		{30, 1, "30"},
		{30000, 1001, "29.97"},
		{60000, 1001, "59.94"},
		{24000, 1001, "23.98"},
		{25, 0, "25"},
		{0, 0, "0"},
		{0, 1, "0"},
		{UINT32_MAX, 1, "4294967295"},
		{1, 3, "0.33"},
		/* characterized, not endorsed: a whole rate with fps_den > 1 keeps
		 * its decimals */
		{50, 2, "25.00"},
	};

	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		fixture_ovi.fps_num = cases[i].num;
		fixture_ovi.fps_den = cases[i].den;
		printf("fps %u/%u\n", cases[i].num, cases[i].den);
		expect_name(NULL, true, "%FPS", cases[i].expected);
	}
}

static void video_format_names_test(void **state)
{
	UNUSED_PARAMETER(state);

	static const struct {
		enum video_format format;
		const char *expected;
	} cases[] = {
		{VIDEO_FORMAT_NONE, "None"}, {VIDEO_FORMAT_I420, "I420"}, {VIDEO_FORMAT_NV12, "NV12"},
		{VIDEO_FORMAT_I444, "I444"}, {VIDEO_FORMAT_P010, "P010"}, {VIDEO_FORMAT_BGRA, "BGRA"},
	};

	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		fixture_ovi.output_format = cases[i].format;
		expect_name(NULL, true, "%VF", cases[i].expected);
	}
}

/* The video info is read once per call, before the format is looked at. With
 * no video, obs_get_video_info returns false, and the fields format as zero
 * whatever the call left in them. */
static void video_info_read_test(void **state)
{
	UNUSED_PARAMETER(state);

	expect_name(NULL, true, "plain", "plain");
	assert_int_equal(video_info_calls, 1);

	expect_name(NULL, true, "%FPS %CRES %FPS", "30 0x0 30");
	assert_int_equal(video_info_calls, 2);

	/* with no video, obs_get_video_info returns false; the real one leaves
	 * the struct unset, the fixture fills it with non-zero fields */
	fixture_result = false;
	fixture_ovi.fps_num = 48;
	fixture_ovi.base_width = 640;
	fixture_ovi.base_height = 360;
	fixture_ovi.output_width = 320;
	fixture_ovi.output_height = 180;
	fixture_ovi.output_format = VIDEO_FORMAT_NV12;
	expect_name(NULL, true, "%FPS %CRES %ORES %VF", "0 0x0 0x0 None");
	assert_int_equal(video_info_calls, 3);
}

/* The table is matched in order and the first entry that matches wins, so a
 * longer OBS specifier is taken before a shorter strftime one; what follows a
 * match is literal text. A replacement is never scanned again. */
static void specifier_matching_test(void **state)
{
	UNUSED_PARAMETER(state);

	static const struct name_case cases[] = {
		{"%YYY", "25Y"},
		{"%CCYYYY", "2025YY"},
		{"%MMM", "11M"},
		{"%mmm", "09m"},
		{"%DDD", "07D"},
		{"%hhh", "16h"},
		{"%sss", "03s"},
		{"%FPSS", "30S"},
		{"%VFF", "NoneF"},
		{"%CRESS", "0x0S"},
		{"%hh%mm%ss", "160903"},
		{"%CCYY-%MM-%DD %hh-%mm-%ss", "2025-11-07 16-09-03"},
		{"x%Yy%yY", "x2025y25Y"},
		{"%%CCYY", "%CCYY"},
		{"%%FPS", "%FPS"},
		/* specifiers are case sensitive */
		{"%fps", "fps"},
		{"%cres", "cres"},
		{"%vf", "vf"},
	};

	expect_names(cases, sizeof(cases) / sizeof(cases[0]));
}

/* %% is a literal %. Any other % that starts no specifier is dropped and the
 * text after it is kept, strftime conversions outside the table included. */
static void percent_and_unknown_specifiers_test(void **state)
{
	UNUSED_PARAMETER(state);

	static const struct name_case cases[] = {
		{"%%", "%"},     {"%%%", "%"},
		{"%%%%", "%%"},  {"100%%", "100%"},
		{"100%", "100"}, {"a%", "a"},
		{"%q", "q"},     {"%Q%q", "Qq"},
		{"%e", "e"},     {"%j", "j"},
		{"%c", "c"},     {"%T", "T"},
		{"%C", "C"},     {"%CCY", "CCY"},
		{"%F", "F"},     {"%FP", "FP"},
		{"%CRE", "CRE"}, {"%ORE", "ORE"},
		{"%V", "V"},     {"%1", "1"},
		{"% ", " "},     {"%\xC3\xA9", "\xC3\xA9"},
	};

	expect_names(cases, sizeof(cases) / sizeof(cases[0]));
}

/* With space false every space of the expanded name becomes an underscore;
 * the extension is appended afterwards and keeps its spaces. Other blanks
 * are kept. */
static void space_replacement_test(void **state)
{
	UNUSED_PARAMETER(state);

	expect_name(NULL, true, " a b  c ", " a b  c ");
	expect_name(NULL, false, " a b  c ", "_a_b__c_");
	expect_name(NULL, false, "   ", "___");
	expect_name(NULL, false, "a\tb", "a\tb");
	expect_name(NULL, false, "%A %B", "Friday_November");
	expect_name("mkv", false, "%CCYY-%MM-%DD %hh-%mm-%ss", "2025-11-07_16-09-03.mkv");
	expect_name("mkv", true, "%CCYY-%MM-%DD %hh-%mm-%ss", "2025-11-07 16-09-03.mkv");
	expect_name("my ext", false, "a b", "a_b.my ext");
	fixture_ovi.base_width = 1920;
	fixture_ovi.base_height = 1080;
	expect_name(NULL, false, "%CRES %FPS", "1920x1080_30");
}

/* A non-empty extension is appended after a dot, as it is: no specifiers,
 * and a leading dot is kept. */
static void extension_test(void **state)
{
	UNUSED_PARAMETER(state);

	expect_name("mkv", true, "name", "name.mkv");
	expect_name("", true, "name", "name");
	expect_name(NULL, true, "name", "name");
	expect_name(".mkv", true, "name", "name..mkv");
	expect_name("%CCYY", true, "name", "name.%CCYY");
	expect_name("tar.gz", true, "name", "name.tar.gz");
}

/* An empty name returns NULL, not "". */
static void empty_format_test(void **state)
{
	UNUSED_PARAMETER(state);

	expect_name(NULL, true, "", NULL);
	expect_name("", true, "", NULL);
	expect_name("mkv", true, "", ".mkv");
	expect_name(NULL, true, NULL, NULL);
	expect_name("mkv", false, NULL, ".mkv");
	/* the name is empty after expansion */
	expect_name(NULL, true, "%", NULL);
	expect_name("mkv", true, "%", ".mkv");
	assert_int_equal(video_info_calls, 7);
}

/* Expects n bytes of 'x' plus format_tail to give n bytes of 'x' plus
 * expected_tail. */
static void expect_long_name(const char *extension, size_t n, const char *format_tail, const char *expected_tail)
{
	struct dstr fmt = {0};
	struct dstr expected = {0};

	for (size_t i = 0; i < n; i++)
		dstr_cat_ch(&fmt, 'x');
	dstr_copy_dstr(&expected, &fmt);
	dstr_cat(&fmt, format_tail);
	dstr_cat(&expected, expected_tail);
	expect_name(extension, true, fmt.array, expected.array);

	dstr_free(&fmt);
	dstr_free(&expected);
}

/* The 255-byte limit applies after expansion and to the extension too. */
static void limit_after_expansion_test(void **state)
{
	UNUSED_PARAMETER(state);

	/* 252 + 4 bytes of %CCYY: the year is cut to "202" */
	expect_long_name(NULL, 252, "%CCYY", "202");
	/* 252 + ".mkv": the extension is cut to ".mk" */
	expect_long_name("mkv", 252, "", ".mk");
	/* 251 + ".mkv" is exactly 255 bytes and kept */
	expect_long_name("mkv", 251, "", ".mkv");
	/* "%%" counts once: 254 and two escapes make 256 bytes */
	expect_long_name(NULL, 254, "%%%%", "%");
}

int main()
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup(zero_width_space_report_test, reset_video),
		cmocka_unit_test_setup(every_interior_cut_test, reset_video),
		cmocka_unit_test_setup(ascii_truncation_unchanged_test, reset_video),
		cmocka_unit_test_setup(invalid_continuation_bytes_test, reset_video),
		cmocka_unit_test_setup(short_name_unchanged_test, reset_video),
		cmocka_unit_test_setup(date_time_specifiers_test, reset_video),
		cmocka_unit_test_setup(unix_time_specifier_test, reset_video),
		cmocka_unit_test_setup(video_specifiers_test, reset_video),
		cmocka_unit_test_setup(fps_formatting_test, reset_video),
		cmocka_unit_test_setup(video_format_names_test, reset_video),
		cmocka_unit_test_setup(video_info_read_test, reset_video),
		cmocka_unit_test_setup(specifier_matching_test, reset_video),
		cmocka_unit_test_setup(percent_and_unknown_specifiers_test, reset_video),
		cmocka_unit_test_setup(space_replacement_test, reset_video),
		cmocka_unit_test_setup(extension_test, reset_video),
		cmocka_unit_test_setup(empty_format_test, reset_video),
		cmocka_unit_test_setup(limit_after_expansion_test, reset_video),
	};

	return cmocka_run_group_tests(tests, fixed_tz_and_locale, NULL);
}
