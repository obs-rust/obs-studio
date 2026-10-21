#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <locale.h>
#include <wchar.h>
#include <cmocka.h>

#include <util/platform.h>
#include <util/bmem.h>

/*
 * Characterization tests for the string conversions in
 * libobs/util/platform-conversions.c (#262 slice 1): os_utf8_to_wcs,
 * os_wcs_to_utf8, os_mbs_to_wcs, os_wcs_to_mbs, their _ptr forms,
 * os_utf8_to_mbs_ptr and os_mbs_to_utf8_ptr. They pin the C behavior for the
 * Rust port. test_platform.c has a few round trips and the #263 cases; this
 * file covers every function on its own.
 *
 * wchar_t is 32 bits on Linux and macOS (one unit per code point) and 16 bits
 * on Windows (UTF-16, a surrogate pair above U+FFFF); WIDE_UTF32 picks the
 * expected units. The UTF-8 conversions go through utf8_to_wchar and
 * wchar_to_utf8, which are libobs's own decoder off Windows and
 * MultiByteToWideChar / WideCharToMultiByte on Windows, so invalid input
 * differs: off Windows it fails, on Windows it becomes U+FFFD. Where only one
 * side is pinned, the other is left to the port's parity test.
 *
 * The mbs conversions use mbstowcs / wcstombs, so they depend on LC_CTYPE.
 * group_setup sets LC_ALL to "C"; the cases that need UTF-8 switch LC_CTYPE
 * to a UTF-8 locale and back, and skip where none is installed.
 *
 * Odd behavior below is marked "characterized, not endorsed".
 */

#define WIDE_UTF32 (WCHAR_MAX > 0xFFFF)

static char *saved_locale;

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);

	const char *cur = setlocale(LC_ALL, NULL);
	saved_locale = cur ? bstrdup(cur) : NULL;
	return setlocale(LC_ALL, "C") ? 0 : -1;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);

	if (saved_locale)
		setlocale(LC_ALL, saved_locale);
	bfree(saved_locale);
	return 0;
}

/* Switches LC_CTYPE to a UTF-8 locale; false where none is installed. */
static bool set_utf8_ctype(void)
{
	static const char *const names[] = {"C.UTF-8", "en_US.UTF-8", ".UTF8", "UTF-8"};
	for (size_t i = 0; i < sizeof(names) / sizeof(names[0]); i++) {
		if (setlocale(LC_CTYPE, names[i]))
			return true;
	}
	return false;
}

static void set_c_ctype(void)
{
	assert_non_null(setlocale(LC_CTYPE, "C"));
}

/* w holds exactly the n units of want, then a NUL */
static void check_wide(const wchar_t *w, const uint32_t *want, size_t n)
{
	assert_non_null(w);
	for (size_t i = 0; i < n; i++)
		assert_int_equal((uint32_t)w[i], want[i]);
	assert_int_equal((uint32_t)w[n], 0);
}

/* s holds exactly the n bytes of want, then a NUL */
static void check_bytes(const char *s, const char *want, size_t n)
{
	assert_non_null(s);
	assert_memory_equal(s, want, n);
	assert_int_equal(s[n], 0);
}

#define FILL_W(buf) wmemset((buf), L'x', sizeof(buf) / sizeof((buf)[0]))
#define FILL_S(buf) memset((buf), 'x', sizeof(buf))

/* ------------------------------------------------------------------------ */
/* os_utf8_to_wcs                                                            */

static void test_utf8_to_wcs_ascii(void **state)
{
	UNUSED_PARAMETER(state);

	static const uint32_t hello[] = {'h', 'e', 'l', 'l', 'o'};
	wchar_t buf[16];

	/* NULL dst: the number of units, without the NUL */
	assert_int_equal(os_utf8_to_wcs("hello", 0, NULL, 0), 5);
	/* dst_size is ignored for a size query */
	assert_int_equal(os_utf8_to_wcs("hello", 0, NULL, 3), 5);

	FILL_W(buf);
	assert_int_equal(os_utf8_to_wcs("hello", 0, buf, 16), 5);
	check_wide(buf, hello, 5);
	assert_int_equal(buf[6], L'x');

	/* len 0 means strlen; an explicit len converts that many bytes */
	FILL_W(buf);
	assert_int_equal(os_utf8_to_wcs("hello", 5, buf, 16), 5);
	check_wide(buf, hello, 5);
	assert_int_equal(os_utf8_to_wcs("hello", 3, NULL, 0), 3);
	FILL_W(buf);
	assert_int_equal(os_utf8_to_wcs("hello", 3, buf, 16), 3);
	check_wide(buf, hello, 3);
}

static void test_utf8_to_wcs_explicit_len_keeps_nul(void **state)
{
	UNUSED_PARAMETER(state);

	/* with an explicit len an embedded NUL is converted like any byte */
	static const uint32_t want[] = {'a', 0, 'b'};
	wchar_t buf[8];

	assert_int_equal(os_utf8_to_wcs("a\0b", 3, NULL, 0), 3);
	FILL_W(buf);
	assert_int_equal(os_utf8_to_wcs("a\0b", 3, buf, 8), 3);
	check_wide(buf, want, 3);

	/* len 0 stops at it */
	assert_int_equal(os_utf8_to_wcs("a\0b", 0, NULL, 0), 1);
}

static void test_utf8_to_wcs_multibyte(void **state)
{
	UNUSED_PARAMETER(state);

	/* 'a', U+00E9 (2 bytes), U+20AC (3 bytes), U+1F600 (4 bytes) */
	const char *utf8 = "a\xC3\xA9\xE2\x82\xAC\xF0\x9F\x98\x80";
#if WIDE_UTF32
	static const uint32_t want[] = {'a', 0x00E9, 0x20AC, 0x1F600};
#else
	static const uint32_t want[] = {'a', 0x00E9, 0x20AC, 0xD83D, 0xDE00};
#endif
	const size_t n = sizeof(want) / sizeof(want[0]);
	wchar_t buf[16];

	assert_int_equal(os_utf8_to_wcs(utf8, 0, NULL, 0), n);
	FILL_W(buf);
	assert_int_equal(os_utf8_to_wcs(utf8, 0, buf, 16), n);
	check_wide(buf, want, n);

	/* one character of each length on its own */
	assert_int_equal(os_utf8_to_wcs("\xC3\xA9", 0, buf, 16), 1);
	assert_int_equal((uint32_t)buf[0], 0x00E9);
	assert_int_equal(os_utf8_to_wcs("\xE2\x82\xAC", 0, buf, 16), 1);
	assert_int_equal((uint32_t)buf[0], 0x20AC);
	assert_int_equal(os_utf8_to_wcs("\xF0\x9F\x98\x80", 0, buf, 16), n - 3);
	check_wide(buf, want + 3, n - 3);

	/* U+FFFF and U+10000, the edges of one and two UTF-16 units */
	assert_int_equal(os_utf8_to_wcs("\xEF\xBF\xBF", 0, buf, 16), 1);
	assert_int_equal((uint32_t)buf[0], 0xFFFF);
#if WIDE_UTF32
	assert_int_equal(os_utf8_to_wcs("\xF0\x90\x80\x80", 0, buf, 16), 1);
	assert_int_equal((uint32_t)buf[0], 0x10000);
#else
	assert_int_equal(os_utf8_to_wcs("\xF0\x90\x80\x80", 0, buf, 16), 2);
	assert_int_equal((uint32_t)buf[0], 0xD800);
	assert_int_equal((uint32_t)buf[1], 0xDC00);
#endif
}

static void test_utf8_to_wcs_bom(void **state)
{
	UNUSED_PARAMETER(state);

	wchar_t buf[8];

	FILL_W(buf);
#ifdef _WIN32
	/* the Windows utf8_to_wchar drops a leading UTF-8 BOM */
	static const uint32_t want[] = {'a'};
	assert_int_equal(os_utf8_to_wcs("\xEF\xBB\xBF"
					"a",
					0, NULL, 0),
			 1);
	assert_int_equal(os_utf8_to_wcs("\xEF\xBB\xBF"
					"a",
					0, buf, 8),
			 1);
	check_wide(buf, want, 1);
#else
	/* off Windows it is kept as U+FEFF */
	static const uint32_t want[] = {0xFEFF, 'a'};
	assert_int_equal(os_utf8_to_wcs("\xEF\xBB\xBF"
					"a",
					0, NULL, 0),
			 2);
	assert_int_equal(os_utf8_to_wcs("\xEF\xBB\xBF"
					"a",
					0, buf, 8),
			 2);
	check_wide(buf, want, 2);
#endif
}

static void test_utf8_to_wcs_invalid(void **state)
{
	UNUSED_PARAMETER(state);

	wchar_t buf[16];

#ifdef _WIN32
	/* MultiByteToWideChar replaces an invalid byte with U+FFFD */
	static const uint32_t want[] = {'b', 'a', 'd', ' ', 0xFFFD, ' ', 'b', 'y', 't', 'e'};
	assert_int_equal(os_utf8_to_wcs("bad \xFF byte", 0, NULL, 0), 10);
	FILL_W(buf);
	assert_int_equal(os_utf8_to_wcs("bad \xFF byte", 0, buf, 16), 10);
	check_wide(buf, want, 10);
#else
	/* any invalid sequence fails the whole string: 0 and an empty dst */
	static const char *const bad[] = {
		"bad \xFF byte", /* forbidden byte */
		"\x80",          /* lone continuation byte */
		"\xC0\xAF",      /* forbidden lead byte (overlong) */
		"\xC3",          /* truncated 2-byte sequence */
		"\xE2\x82",      /* truncated 3-byte sequence */
		"\xE2\x82x",     /* bad continuation byte */
		"\xFE",          /* not a lead byte */
	};
	for (size_t i = 0; i < sizeof(bad) / sizeof(bad[0]); i++) {
		assert_int_equal(os_utf8_to_wcs(bad[i], 0, NULL, 0), 0);
		FILL_W(buf);
		assert_int_equal(os_utf8_to_wcs(bad[i], 0, buf, 16), 0);
		assert_int_equal(buf[0], 0);
	}

	/* an explicit len that cuts a sequence makes it truncated */
	assert_int_equal(os_utf8_to_wcs("caf\xC3\xA9", 4, NULL, 0), 0);
	FILL_W(buf);
	assert_int_equal(os_utf8_to_wcs("caf\xC3\xA9", 4, buf, 16), 0);
	assert_int_equal(buf[0], 0);

	/*
	 * Characterized, not endorsed: an encoded surrogate (U+D800) passes the
	 * size query, which does not check the decoded value, then fails the
	 * conversion. So the size query says 1 and the conversion gives 0.
	 */
	assert_int_equal(os_utf8_to_wcs("\xED\xA0\x80", 0, NULL, 0), 1);
	FILL_W(buf);
	assert_int_equal(os_utf8_to_wcs("\xED\xA0\x80", 0, buf, 16), 0);
	assert_int_equal(buf[0], 0);

	/*
	 * Characterized, not endorsed: the decoder does not reject overlong
	 * forms past 2 bytes, code points above U+10FFFF, or 5- and 6-byte
	 * sequences.
	 */
	assert_int_equal(os_utf8_to_wcs("\xE0\x80\xAF", 0, buf, 16), 1); /* overlong '/' */
	assert_int_equal((uint32_t)buf[0], '/');
	assert_int_equal(os_utf8_to_wcs("\xF4\x90\x80\x80", 0, buf, 16), 1);
	assert_int_equal((uint32_t)buf[0], 0x110000);
	assert_int_equal(os_utf8_to_wcs("\xF8\x88\x80\x80\x80", 0, buf, 16), 1);
	assert_int_equal((uint32_t)buf[0], 0x200000);
	assert_int_equal(os_utf8_to_wcs("\xFC\x84\x80\x80\x80\x80", 0, buf, 16), 1);
	assert_int_equal((uint32_t)buf[0], 0x4000000);
#endif
}

static void test_utf8_to_wcs_dst_size(void **state)
{
	UNUSED_PARAMETER(state);

	static const uint32_t hello[] = {'h', 'e', 'l', 'l', 'o'};
	struct {
		wchar_t w[8];
		wchar_t guard;
	} b;

	/* NULL str: 0, dst untouched */
	FILL_W(b.w);
	assert_int_equal(os_utf8_to_wcs(NULL, 0, b.w, 8), 0);
	assert_int_equal(b.w[0], L'x');
	assert_int_equal(os_utf8_to_wcs(NULL, 0, NULL, 0), 0);

	/* dst_size 0: 0, nothing written */
	FILL_W(b.w);
	assert_int_equal(os_utf8_to_wcs("hello", 0, b.w, 0), 0);
	assert_int_equal(b.w[0], L'x');

	/* dst_size 1: room for the NUL only */
	FILL_W(b.w);
	assert_int_equal(os_utf8_to_wcs("hello", 0, b.w, 1), 0);
	assert_int_equal(b.w[0], 0);
	assert_int_equal(b.w[1], L'x');

	/* exactly the text plus its NUL */
	FILL_W(b.w);
	b.guard = L'G';
	assert_int_equal(os_utf8_to_wcs("hello", 0, b.w, 6), 5);
	check_wide(b.w, hello, 5);
	assert_int_equal(b.w[6], L'x');

	/*
	 * Characterized, not endorsed: a dst too small for the text gives 0 and
	 * an empty string, not the text cut to fit (the mbs conversions cut).
	 */
	FILL_W(b.w);
	b.guard = L'G';
	assert_int_equal(os_utf8_to_wcs("hello world", 0, b.w, 4), 0);
	assert_int_equal(b.w[0], 0);
	assert_int_equal(b.w[4], L'x');
	assert_int_equal(b.guard, L'G');

	FILL_W(b.w);
	assert_int_equal(os_utf8_to_wcs("a\xC3\xA9\xE2\x82\xAC"
					"bcdefghij",
					0, b.w, 3),
			 0);
	assert_int_equal(b.w[0], 0);
	assert_int_equal(b.w[3], L'x');
}

static void test_utf8_to_wcs_empty(void **state)
{
	UNUSED_PARAMETER(state);

	wchar_t buf[4];

	assert_int_equal(os_utf8_to_wcs("", 0, NULL, 0), 0);
	FILL_W(buf);
	assert_int_equal(os_utf8_to_wcs("", 0, buf, 4), 0);
	assert_int_equal(buf[0], 0);
	assert_int_equal(buf[1], L'x');
}

/* ------------------------------------------------------------------------ */
/* os_wcs_to_utf8                                                            */

static void test_wcs_to_utf8_ascii(void **state)
{
	UNUSED_PARAMETER(state);

	char buf[16];

	assert_int_equal(os_wcs_to_utf8(L"hello", 0, NULL, 0), 5);
	assert_int_equal(os_wcs_to_utf8(L"hello", 0, NULL, 2), 5);

	FILL_S(buf);
	assert_int_equal(os_wcs_to_utf8(L"hello", 0, buf, 16), 5);
	check_bytes(buf, "hello", 5);
	assert_int_equal(buf[6], 'x');

	assert_int_equal(os_wcs_to_utf8(L"hello", 5, NULL, 0), 5);
	assert_int_equal(os_wcs_to_utf8(L"hello", 3, NULL, 0), 3);
	FILL_S(buf);
	assert_int_equal(os_wcs_to_utf8(L"hello", 3, buf, 16), 3);
	check_bytes(buf, "hel", 3);

	/* with an explicit len an embedded NUL is converted like any unit */
	static const wchar_t anb[] = {L'a', 0, L'b', 0};
	FILL_S(buf);
	assert_int_equal(os_wcs_to_utf8(anb, 3, buf, 16), 3);
	check_bytes(buf, "a\0b", 3);
	assert_int_equal(os_wcs_to_utf8(anb, 0, NULL, 0), 1);
}

static void test_wcs_to_utf8_multibyte(void **state)
{
	UNUSED_PARAMETER(state);

#if WIDE_UTF32
	static const wchar_t wide[] = {L'a', (wchar_t)0x00E9, (wchar_t)0x20AC, (wchar_t)0x1F600, 0};
	static const wchar_t smile[] = {(wchar_t)0x1F600, 0};
#else
	static const wchar_t wide[] = {L'a', (wchar_t)0x00E9, (wchar_t)0x20AC, (wchar_t)0xD83D, (wchar_t)0xDE00, 0};
	static const wchar_t smile[] = {(wchar_t)0xD83D, (wchar_t)0xDE00, 0};
#endif
	static const wchar_t e_acute[] = {(wchar_t)0x00E9, 0};
	static const wchar_t euro[] = {(wchar_t)0x20AC, 0};
	const char *utf8 = "a\xC3\xA9\xE2\x82\xAC\xF0\x9F\x98\x80";
	char buf[16];

	assert_int_equal(os_wcs_to_utf8(wide, 0, NULL, 0), 10);
	FILL_S(buf);
	assert_int_equal(os_wcs_to_utf8(wide, 0, buf, 16), 10);
	check_bytes(buf, utf8, 10);

	assert_int_equal(os_wcs_to_utf8(e_acute, 0, buf, 16), 2);
	check_bytes(buf, "\xC3\xA9", 2);
	assert_int_equal(os_wcs_to_utf8(euro, 0, buf, 16), 3);
	check_bytes(buf, "\xE2\x82\xAC", 3);
	assert_int_equal(os_wcs_to_utf8(smile, 0, buf, 16), 4);
	check_bytes(buf, "\xF0\x9F\x98\x80", 4);
}

static void test_wcs_to_utf8_invalid(void **state)
{
	UNUSED_PARAMETER(state);

	static const wchar_t lone[] = {L'a', (wchar_t)0xD800, L'b', 0};
	char buf[16];

#ifdef _WIN32
	/* WideCharToMultiByte replaces a lone surrogate with U+FFFD */
	assert_int_equal(os_wcs_to_utf8(lone, 0, NULL, 0), 5);
	FILL_S(buf);
	assert_int_equal(os_wcs_to_utf8(lone, 0, buf, 16), 5);
	check_bytes(buf,
		    "a\xEF\xBF\xBD"
		    "b",
		    5);

	/* an explicit len that splits a surrogate pair leaves a lone one */
	static const wchar_t pair[] = {(wchar_t)0xD83D, (wchar_t)0xDE00, 0};
	assert_int_equal(os_wcs_to_utf8(pair, 1, buf, 16), 3);
	check_bytes(buf, "\xEF\xBF\xBD", 3);
#else
	/* a surrogate fails the whole string: 0 and an empty dst */
	assert_int_equal(os_wcs_to_utf8(lone, 0, NULL, 0), 0);
	FILL_S(buf);
	assert_int_equal(os_wcs_to_utf8(lone, 0, buf, 16), 0);
	assert_int_equal(buf[0], 0);

	static const wchar_t pair[] = {(wchar_t)0xD83D, (wchar_t)0xDE00, 0};
	assert_int_equal(os_wcs_to_utf8(pair, 0, NULL, 0), 0);
	FILL_S(buf);
	assert_int_equal(os_wcs_to_utf8(pair, 0, buf, 16), 0);
	assert_int_equal(buf[0], 0);

	/*
	 * Characterized, not endorsed: values above U+10FFFF are encoded in
	 * 4 to 6 bytes instead of being refused.
	 */
	static const wchar_t big[] = {(wchar_t)0x110000, 0};
	static const wchar_t bigger[] = {(wchar_t)0x7FFFFFFF, 0};
	assert_int_equal(os_wcs_to_utf8(big, 0, buf, 16), 4);
	check_bytes(buf, "\xF4\x90\x80\x80", 4);
	assert_int_equal(os_wcs_to_utf8(bigger, 0, buf, 16), 6);
	check_bytes(buf, "\xFD\xBF\xBF\xBF\xBF\xBF", 6);
#endif
}

static void test_wcs_to_utf8_dst_size(void **state)
{
	UNUSED_PARAMETER(state);

	static const wchar_t ae[] = {L'a', (wchar_t)0x00E9, 0};
	struct {
		char s[8];
		char guard;
	} b;

	FILL_S(b.s);
	assert_int_equal(os_wcs_to_utf8(NULL, 0, b.s, 8), 0);
	assert_int_equal(b.s[0], 'x');
	assert_int_equal(os_wcs_to_utf8(NULL, 0, NULL, 0), 0);

	FILL_S(b.s);
	assert_int_equal(os_wcs_to_utf8(L"hello", 0, b.s, 0), 0);
	assert_int_equal(b.s[0], 'x');

	FILL_S(b.s);
	assert_int_equal(os_wcs_to_utf8(L"hello", 0, b.s, 1), 0);
	assert_int_equal(b.s[0], 0);
	assert_int_equal(b.s[1], 'x');

	/* exactly the text plus its NUL */
	FILL_S(b.s);
	assert_int_equal(os_wcs_to_utf8(L"hello", 0, b.s, 6), 5);
	check_bytes(b.s, "hello", 5);
	assert_int_equal(b.s[6], 'x');

	/*
	 * Characterized, not endorsed: one byte short, or a multibyte character
	 * that does not fit, gives 0 and an empty string, not the text cut.
	 */
	FILL_S(b.s);
	b.guard = 'G';
	assert_int_equal(os_wcs_to_utf8(L"hello", 0, b.s, 5), 0);
	assert_int_equal(b.s[0], 0);
	assert_int_equal(b.s[5], 'x');

	FILL_S(b.s);
	assert_int_equal(os_wcs_to_utf8(ae, 0, b.s, 3), 0);
	assert_int_equal(b.s[0], 0);
	assert_int_equal(b.s[3], 'x');
	assert_int_equal(os_wcs_to_utf8(ae, 0, b.s, 4), 3);
	check_bytes(b.s, "a\xC3\xA9", 3);

	FILL_S(b.s);
	b.guard = 'G';
	assert_int_equal(os_wcs_to_utf8(L"hello world", 0, b.s, 8), 0);
	assert_int_equal(b.s[0], 0);
	assert_int_equal(b.guard, 'G');
}

static void test_wcs_to_utf8_empty(void **state)
{
	UNUSED_PARAMETER(state);

	char buf[4];

	assert_int_equal(os_wcs_to_utf8(L"", 0, NULL, 0), 0);
	FILL_S(buf);
	assert_int_equal(os_wcs_to_utf8(L"", 0, buf, 4), 0);
	assert_int_equal(buf[0], 0);
	assert_int_equal(buf[1], 'x');
}

/* ------------------------------------------------------------------------ */
/* os_mbs_to_wcs and os_wcs_to_mbs (LC_CTYPE)                                */

static void test_mbs_to_wcs_c_locale(void **state)
{
	UNUSED_PARAMETER(state);

	static const uint32_t hello[] = {'h', 'e', 'l', 'l', 'o'};
	struct {
		wchar_t w[8];
		wchar_t guard;
	} b;

	set_c_ctype();

	assert_int_equal(os_mbs_to_wcs("hello", 0, NULL, 0), 5);
	FILL_W(b.w);
	assert_int_equal(os_mbs_to_wcs("hello", 0, b.w, 8), 5);
	check_wide(b.w, hello, 5);
	assert_int_equal(b.w[6], L'x');

	/* exact fit */
	FILL_W(b.w);
	assert_int_equal(os_mbs_to_wcs("hello", 0, b.w, 6), 5);
	check_wide(b.w, hello, 5);

	/* Characterized, not endorsed: len is ignored, the whole string is converted */
	assert_int_equal(os_mbs_to_wcs("hello", 3, NULL, 0), 5);
	FILL_W(b.w);
	assert_int_equal(os_mbs_to_wcs("hello", 3, b.w, 8), 5);
	check_wide(b.w, hello, 5);

	/* too small: cut to dst_size - 1 units, unlike os_utf8_to_wcs (#263) */
	FILL_W(b.w);
	b.guard = L'G';
	assert_int_equal(os_mbs_to_wcs("hello world", 0, b.w, 8), 7);
	static const uint32_t cut[] = {'h', 'e', 'l', 'l', 'o', ' ', 'w'};
	check_wide(b.w, cut, 7);
	assert_int_equal(b.guard, L'G');

	FILL_W(b.w);
	assert_int_equal(os_mbs_to_wcs("hello", 0, b.w, 1), 0);
	assert_int_equal(b.w[0], 0);
	assert_int_equal(b.w[1], L'x');

	FILL_W(b.w);
	assert_int_equal(os_mbs_to_wcs("hello", 0, b.w, 0), 0);
	assert_int_equal(b.w[0], L'x');

	/* NULL and empty */
	FILL_W(b.w);
	assert_int_equal(os_mbs_to_wcs(NULL, 0, b.w, 8), 0);
	assert_int_equal(b.w[0], L'x');
	assert_int_equal(os_mbs_to_wcs(NULL, 0, NULL, 0), 0);
	assert_int_equal(os_mbs_to_wcs("", 0, NULL, 0), 0);
	FILL_W(b.w);
	assert_int_equal(os_mbs_to_wcs("", 0, b.w, 8), 0);
	assert_int_equal(b.w[0], 0);
	assert_int_equal(b.w[1], L'x');
}

static void test_wcs_to_mbs_c_locale(void **state)
{
	UNUSED_PARAMETER(state);

	/* U+4E2D has no byte in the C locale, on every platform */
	static const wchar_t han[] = {L'a', (wchar_t)0x4E2D, 0};
	struct {
		char s[8];
		char guard;
	} b;

	set_c_ctype();

	assert_int_equal(os_wcs_to_mbs(L"hello", 0, NULL, 0), 5);
	FILL_S(b.s);
	assert_int_equal(os_wcs_to_mbs(L"hello", 0, b.s, 8), 5);
	check_bytes(b.s, "hello", 5);
	assert_int_equal(b.s[6], 'x');

	FILL_S(b.s);
	assert_int_equal(os_wcs_to_mbs(L"hello", 0, b.s, 6), 5);
	check_bytes(b.s, "hello", 5);

	/* Characterized, not endorsed: len is ignored */
	assert_int_equal(os_wcs_to_mbs(L"hello", 3, NULL, 0), 5);
	FILL_S(b.s);
	assert_int_equal(os_wcs_to_mbs(L"hello", 3, b.s, 8), 5);
	check_bytes(b.s, "hello", 5);

	/* too small: cut to dst_size - 1 bytes (#263) */
	FILL_S(b.s);
	b.guard = 'G';
	assert_int_equal(os_wcs_to_mbs(L"hello world", 0, b.s, 8), 7);
	check_bytes(b.s, "hello w", 7);
	assert_int_equal(b.guard, 'G');

	FILL_S(b.s);
	assert_int_equal(os_wcs_to_mbs(L"hello", 0, b.s, 1), 0);
	assert_int_equal(b.s[0], 0);
	assert_int_equal(b.s[1], 'x');

	FILL_S(b.s);
	assert_int_equal(os_wcs_to_mbs(L"hello", 0, b.s, 0), 0);
	assert_int_equal(b.s[0], 'x');

	/* unconvertible: 0 and an empty dst (#263) */
	assert_int_equal(os_wcs_to_mbs(han, 0, NULL, 0), 0);
	FILL_S(b.s);
	assert_int_equal(os_wcs_to_mbs(han, 0, b.s, 8), 0);
	assert_int_equal(b.s[0], 0);

	FILL_S(b.s);
	assert_int_equal(os_wcs_to_mbs(NULL, 0, b.s, 8), 0);
	assert_int_equal(b.s[0], 'x');
	assert_int_equal(os_wcs_to_mbs(NULL, 0, NULL, 0), 0);
	assert_int_equal(os_wcs_to_mbs(L"", 0, NULL, 0), 0);
	FILL_S(b.s);
	assert_int_equal(os_wcs_to_mbs(L"", 0, b.s, 8), 0);
	assert_int_equal(b.s[0], 0);
}

static void test_mbs_utf8_locale(void **state)
{
	UNUSED_PARAMETER(state);

	static const uint32_t cafe[] = {'c', 'a', 'f', 0x00E9};
	static const wchar_t wcafe[] = {L'c', L'a', L'f', (wchar_t)0x00E9, 0};
	wchar_t w[8];
	char s[8];

	if (!set_utf8_ctype()) {
		print_message("no UTF-8 locale installed\n");
		skip();
	}

	assert_int_equal(os_mbs_to_wcs("caf\xC3\xA9", 0, NULL, 0), 4);
	FILL_W(w);
	assert_int_equal(os_mbs_to_wcs("caf\xC3\xA9", 0, w, 8), 4);
	check_wide(w, cafe, 4);

	assert_int_equal(os_wcs_to_mbs(wcafe, 0, NULL, 0), 5);
	FILL_S(s);
	assert_int_equal(os_wcs_to_mbs(wcafe, 0, s, 8), 5);
	check_bytes(s, "caf\xC3\xA9", 5);

	/* invalid UTF-8: 0 and an empty dst (#263) */
	assert_int_equal(os_mbs_to_wcs("bad \xFF byte", 0, NULL, 0), 0);
	FILL_W(w);
	assert_int_equal(os_mbs_to_wcs("bad \xFF byte", 0, w, 8), 0);
	assert_int_equal(w[0], 0);

#ifndef _WIN32
	/* a cut never splits a character */
	FILL_W(w);
	assert_int_equal(os_mbs_to_wcs("\xC3\xA9\xC3\xA9\xC3\xA9", 0, w, 3), 2);
	static const uint32_t two[] = {0x00E9, 0x00E9};
	check_wide(w, two, 2);

	FILL_S(s);
	assert_int_equal(os_wcs_to_mbs(wcafe, 0, s, 5), 3);
	check_bytes(s, "caf", 3);
	FILL_S(s);
	assert_int_equal(os_wcs_to_mbs(wcafe, 0, s, 6), 5);
	check_bytes(s, "caf\xC3\xA9", 5);
#endif

#if WIDE_UTF32
	assert_int_equal(os_mbs_to_wcs("\xF0\x9F\x98\x80", 0, w, 8), 1);
	assert_int_equal((uint32_t)w[0], 0x1F600);
	static const wchar_t smile[] = {(wchar_t)0x1F600, 0};
	assert_int_equal(os_wcs_to_mbs(smile, 0, s, 8), 4);
	check_bytes(s, "\xF0\x9F\x98\x80", 4);
#endif

	set_c_ctype();
}

/* ------------------------------------------------------------------------ */
/* the _ptr forms                                                            */

static void test_utf8_wcs_ptr(void **state)
{
	UNUSED_PARAMETER(state);

	static const uint32_t cafe[] = {'c', 'a', 'f', 0x00E9};
	static const wchar_t wcafe[] = {L'c', L'a', L'f', (wchar_t)0x00E9, 0};
	long allocs = bnum_allocs();
	wchar_t *w;
	char *s;

	/* one allocation, freed with bfree */
	w = (wchar_t *)0x1;
	assert_int_equal(os_utf8_to_wcs_ptr("caf\xC3\xA9", 0, &w), 4);
	check_wide(w, cafe, 4);
	assert_int_equal(bnum_allocs(), allocs + 1);
	bfree(w);
	assert_int_equal(bnum_allocs(), allocs);

	assert_int_equal(os_utf8_to_wcs_ptr("caf\xC3\xA9", 3, &w), 3);
	check_wide(w, cafe, 3);
	bfree(w);

	s = (char *)0x1;
	assert_int_equal(os_wcs_to_utf8_ptr(wcafe, 0, &s), 5);
	check_bytes(s, "caf\xC3\xA9", 5);
	assert_int_equal(bnum_allocs(), allocs + 1);
	bfree(s);
	assert_int_equal(bnum_allocs(), allocs);

	assert_int_equal(os_wcs_to_utf8_ptr(wcafe, 3, &s), 3);
	check_bytes(s, "caf", 3);
	bfree(s);

	/* empty: an allocated empty string */
	assert_int_equal(os_utf8_to_wcs_ptr("", 0, &w), 0);
	check_wide(w, NULL, 0);
	bfree(w);
	assert_int_equal(os_wcs_to_utf8_ptr(L"", 0, &s), 0);
	check_bytes(s, "", 0);
	bfree(s);

	/* NULL: *pstr is set to NULL, nothing allocated */
	w = (wchar_t *)0x1;
	s = (char *)0x1;
	assert_int_equal(os_utf8_to_wcs_ptr(NULL, 0, &w), 0);
	assert_null(w);
	assert_int_equal(os_wcs_to_utf8_ptr(NULL, 0, &s), 0);
	assert_null(s);
	assert_int_equal(bnum_allocs(), allocs);

#ifndef _WIN32
	/* invalid input: 0 and an allocated empty string */
	static const wchar_t lone[] = {(wchar_t)0xD800, 0};
	assert_int_equal(os_utf8_to_wcs_ptr("bad \xFF byte", 0, &w), 0);
	check_wide(w, NULL, 0);
	bfree(w);
	assert_int_equal(os_utf8_to_wcs_ptr("\xED\xA0\x80", 0, &w), 0);
	check_wide(w, NULL, 0);
	bfree(w);
	assert_int_equal(os_wcs_to_utf8_ptr(lone, 0, &s), 0);
	check_bytes(s, "", 0);
	bfree(s);
	assert_int_equal(bnum_allocs(), allocs);
#endif
}

static void test_mbs_wcs_ptr(void **state)
{
	UNUSED_PARAMETER(state);

	static const uint32_t hello[] = {'h', 'e', 'l', 'l', 'o'};
	static const wchar_t han[] = {L'a', (wchar_t)0x4E2D, 0};
	long allocs = bnum_allocs();
	wchar_t *w;
	char *s;

	set_c_ctype();

	w = (wchar_t *)0x1;
	assert_int_equal(os_mbs_to_wcs_ptr("hello", 0, &w), 5);
	check_wide(w, hello, 5);
	assert_int_equal(bnum_allocs(), allocs + 1);
	bfree(w);

	/* Characterized, not endorsed: len is ignored */
	assert_int_equal(os_mbs_to_wcs_ptr("hello", 3, &w), 5);
	check_wide(w, hello, 5);
	bfree(w);

	s = (char *)0x1;
	assert_int_equal(os_wcs_to_mbs_ptr(L"hello", 0, &s), 5);
	check_bytes(s, "hello", 5);
	assert_int_equal(bnum_allocs(), allocs + 1);
	bfree(s);
	assert_int_equal(os_wcs_to_mbs_ptr(L"hello", 3, &s), 5);
	check_bytes(s, "hello", 5);
	bfree(s);

	assert_int_equal(os_mbs_to_wcs_ptr("", 0, &w), 0);
	check_wide(w, NULL, 0);
	bfree(w);
	assert_int_equal(os_wcs_to_mbs_ptr(L"", 0, &s), 0);
	check_bytes(s, "", 0);
	bfree(s);

	/* unconvertible: 0 and an allocated empty string (#263) */
	assert_int_equal(os_wcs_to_mbs_ptr(han, 0, &s), 0);
	check_bytes(s, "", 0);
	bfree(s);

	w = (wchar_t *)0x1;
	s = (char *)0x1;
	assert_int_equal(os_mbs_to_wcs_ptr(NULL, 0, &w), 0);
	assert_null(w);
	assert_int_equal(os_wcs_to_mbs_ptr(NULL, 0, &s), 0);
	assert_null(s);
	assert_int_equal(bnum_allocs(), allocs);
}

static void test_utf8_mbs_ptr(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	char *s;

	set_c_ctype();

	/* the wide buffer in between is freed: one allocation left */
	s = (char *)0x1;
	assert_int_equal(os_utf8_to_mbs_ptr("hello", 0, &s), 5);
	check_bytes(s, "hello", 5);
	assert_int_equal(bnum_allocs(), allocs + 1);
	bfree(s);
	assert_int_equal(bnum_allocs(), allocs);

	s = (char *)0x1;
	assert_int_equal(os_mbs_to_utf8_ptr("hello", 0, &s), 5);
	check_bytes(s, "hello", 5);
	assert_int_equal(bnum_allocs(), allocs + 1);
	bfree(s);
	assert_int_equal(bnum_allocs(), allocs);

	/* os_utf8_to_mbs_ptr honors len; os_mbs_to_utf8_ptr ignores it
	 * (characterized, not endorsed) */
	assert_int_equal(os_utf8_to_mbs_ptr("hello", 3, &s), 3);
	check_bytes(s, "hel", 3);
	bfree(s);
	assert_int_equal(os_mbs_to_utf8_ptr("hello", 3, &s), 5);
	check_bytes(s, "hello", 5);
	bfree(s);

	/* empty */
	assert_int_equal(os_utf8_to_mbs_ptr("", 0, &s), 0);
	check_bytes(s, "", 0);
	bfree(s);
	assert_int_equal(os_mbs_to_utf8_ptr("", 0, &s), 0);
	check_bytes(s, "", 0);
	bfree(s);

	/* text the C locale cannot hold: 0 and an allocated empty string */
	assert_int_equal(os_utf8_to_mbs_ptr("a\xE4\xB8\xAD", 0, &s), 0);
	check_bytes(s, "", 0);
	bfree(s);

	/* NULL */
	s = (char *)0x1;
	assert_int_equal(os_utf8_to_mbs_ptr(NULL, 0, &s), 0);
	assert_null(s);
	s = (char *)0x1;
	assert_int_equal(os_mbs_to_utf8_ptr(NULL, 0, &s), 0);
	assert_null(s);
	assert_int_equal(bnum_allocs(), allocs);

	if (set_utf8_ctype()) {
		assert_int_equal(os_utf8_to_mbs_ptr("caf\xC3\xA9", 0, &s), 5);
		check_bytes(s, "caf\xC3\xA9", 5);
		bfree(s);
		assert_int_equal(os_mbs_to_utf8_ptr("caf\xC3\xA9", 0, &s), 5);
		check_bytes(s, "caf\xC3\xA9", 5);
		bfree(s);

		/* invalid UTF-8 on the mbs side (#263) */
		assert_int_equal(os_mbs_to_utf8_ptr("bad \xFF byte", 0, &s), 0);
		check_bytes(s, "", 0);
		bfree(s);
#ifndef _WIN32
		assert_int_equal(os_utf8_to_mbs_ptr("bad \xFF byte", 0, &s), 0);
		check_bytes(s, "", 0);
		bfree(s);
#endif
		assert_int_equal(bnum_allocs(), allocs);
		set_c_ctype();
	} else {
		print_message("no UTF-8 locale: UTF-8 locale cases skipped\n");
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_utf8_to_wcs_ascii),     cmocka_unit_test(test_utf8_to_wcs_explicit_len_keeps_nul),
		cmocka_unit_test(test_utf8_to_wcs_multibyte), cmocka_unit_test(test_utf8_to_wcs_bom),
		cmocka_unit_test(test_utf8_to_wcs_invalid),   cmocka_unit_test(test_utf8_to_wcs_dst_size),
		cmocka_unit_test(test_utf8_to_wcs_empty),     cmocka_unit_test(test_wcs_to_utf8_ascii),
		cmocka_unit_test(test_wcs_to_utf8_multibyte), cmocka_unit_test(test_wcs_to_utf8_invalid),
		cmocka_unit_test(test_wcs_to_utf8_dst_size),  cmocka_unit_test(test_wcs_to_utf8_empty),
		cmocka_unit_test(test_mbs_to_wcs_c_locale),   cmocka_unit_test(test_wcs_to_mbs_c_locale),
		cmocka_unit_test(test_mbs_utf8_locale),       cmocka_unit_test(test_utf8_wcs_ptr),
		cmocka_unit_test(test_mbs_wcs_ptr),           cmocka_unit_test(test_utf8_mbs_ptr),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
