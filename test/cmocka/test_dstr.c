#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <locale.h>
#include <wchar.h>
#include <wctype.h>
#include <cmocka.h>

#include <util/bmem.h>
#include <util/dstr.h>
#include <util/lexer.h>

/* Characterization tests: every assertion pins what libobs/util/dstr.c does
 * today. Each test is bracketed by an allocation-count check to catch leaks. */

static long allocs_at_start;

static int leak_setup(void **state)
{
	UNUSED_PARAMETER(state);
	allocs_at_start = bnum_allocs();
	return 0;
}

static int leak_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	assert_int_equal(bnum_allocs(), allocs_at_start);
	return 0;
}

static void test_astrcmpi(void **state)
{
	UNUSED_PARAMETER(state);

	assert_int_equal(astrcmpi("abc", "abc"), 0);
	assert_int_equal(astrcmpi("abc", "ABC"), 0);
	assert_int_equal(astrcmpi("aBc", "AbC"), 0);
	assert_int_equal(astrcmpi("a", "b"), -1);
	assert_int_equal(astrcmpi("B", "a"), 1);
	assert_int_equal(astrcmpi("ab", "abc"), -1);
	assert_int_equal(astrcmpi("abc", "ab"), 1);
	assert_int_equal(astrcmpi("", ""), 0);

	/* NULL is treated as the empty string */
	assert_int_equal(astrcmpi(NULL, NULL), 0);
	assert_int_equal(astrcmpi(NULL, ""), 0);
	assert_int_equal(astrcmpi("", NULL), 0);
	assert_int_equal(astrcmpi(NULL, "a"), -1);
	assert_int_equal(astrcmpi("a", NULL), 1);
}

static void test_astrcmp_n(void **state)
{
	UNUSED_PARAMETER(state);

	/* n == 0 always equal */
	assert_int_equal(astrcmp_n("a", "b", 0), 0);
	assert_int_equal(astrcmp_n(NULL, "b", 0), 0);

	assert_int_equal(astrcmp_n("abcd", "abce", 3), 0);
	assert_int_equal(astrcmp_n("abcd", "abce", 4), -1);
	assert_int_equal(astrcmp_n("abce", "abcd", 4), 1);
	assert_int_equal(astrcmp_n("abc", "abc", 100), 0);
	assert_int_equal(astrcmp_n("ab", "abc", 100), -1);

	/* case sensitive */
	assert_int_equal(astrcmp_n("A", "a", 1), -1);
	assert_int_equal(astrcmp_n("a", "A", 1), 1);

	/* NULL is treated as the empty string */
	assert_int_equal(astrcmp_n(NULL, NULL, 5), 0);
	assert_int_equal(astrcmp_n(NULL, "a", 1), -1);
	assert_int_equal(astrcmp_n("a", NULL, 1), 1);
}

static void test_astrcmpi_n(void **state)
{
	UNUSED_PARAMETER(state);

	assert_int_equal(astrcmpi_n("a", "b", 0), 0);
	assert_int_equal(astrcmpi_n(NULL, "b", 0), 0);

	assert_int_equal(astrcmpi_n("ABCD", "abce", 3), 0);
	assert_int_equal(astrcmpi_n("ABCD", "abce", 4), -1);
	assert_int_equal(astrcmpi_n("abce", "ABCD", 4), 1);
	assert_int_equal(astrcmpi_n("ABC", "abc", 100), 0);
	assert_int_equal(astrcmpi_n("AB", "abc", 100), -1);

	assert_int_equal(astrcmpi_n(NULL, NULL, 5), 0);
	assert_int_equal(astrcmpi_n(NULL, "a", 1), -1);
	assert_int_equal(astrcmpi_n("a", NULL, 1), 1);
}

static void test_astrstri(void **state)
{
	UNUSED_PARAMETER(state);

	const char *str = "Hello World";

	assert_ptr_equal(astrstri(str, "WORLD"), str + 6);
	assert_ptr_equal(astrstri(str, "llo"), str + 2);
	assert_ptr_equal(astrstri(str, "hello world"), str);
	assert_ptr_equal(astrstri(str, "d"), str + 10);
	assert_null(astrstri(str, "xyz"));
	assert_null(astrstri(str, "Hello World!"));
	assert_null(astrstri("", "a"));

	/* empty needle matches at the start */
	assert_ptr_equal(astrstri(str, ""), str);

	assert_null(astrstri(NULL, "a"));
	assert_null(astrstri(str, NULL));
	assert_null(astrstri(NULL, NULL));
}

static void test_strdepad(void **state)
{
	UNUSED_PARAMETER(state);

	char buf[64];

	strcpy(buf, "  \t hi there \r\n");
	assert_ptr_equal(strdepad(buf), buf);
	assert_string_equal(buf, "hi there");

	strcpy(buf, "nopad");
	assert_ptr_equal(strdepad(buf), buf);
	assert_string_equal(buf, "nopad");

	strcpy(buf, "lead");
	memmove(buf + 3, buf, 5);
	memset(buf, ' ', 3);
	strdepad(buf);
	assert_string_equal(buf, "lead");

	strcpy(buf, "trail \t\n");
	strdepad(buf);
	assert_string_equal(buf, "trail");

	strcpy(buf, " \t\r\n ");
	assert_ptr_equal(strdepad(buf), buf);
	assert_string_equal(buf, "");

	strcpy(buf, "");
	assert_ptr_equal(strdepad(buf), buf);
	assert_string_equal(buf, "");

	assert_null(strdepad(NULL));
}

static void test_strlist_split(void **state)
{
	UNUSED_PARAMETER(state);

	char **list;

	list = strlist_split("a,b,,c", ',', true);
	assert_non_null(list);
	assert_string_equal(list[0], "a");
	assert_string_equal(list[1], "b");
	assert_string_equal(list[2], "");
	assert_string_equal(list[3], "c");
	assert_null(list[4]);
	strlist_free(list);

	list = strlist_split("a,b,,c", ',', false);
	assert_non_null(list);
	assert_string_equal(list[0], "a");
	assert_string_equal(list[1], "b");
	assert_string_equal(list[2], "c");
	assert_null(list[3]);
	strlist_free(list);

	/* trailing separator */
	list = strlist_split("a,b,", ',', true);
	assert_non_null(list);
	assert_string_equal(list[0], "a");
	assert_string_equal(list[1], "b");
	assert_string_equal(list[2], "");
	assert_null(list[3]);
	strlist_free(list);

	list = strlist_split("a,b,", ',', false);
	assert_non_null(list);
	assert_string_equal(list[0], "a");
	assert_string_equal(list[1], "b");
	assert_null(list[2]);
	strlist_free(list);

	/* leading separator */
	list = strlist_split(",a", ',', true);
	assert_non_null(list);
	assert_string_equal(list[0], "");
	assert_string_equal(list[1], "a");
	assert_null(list[2]);
	strlist_free(list);

	/* no separator present */
	list = strlist_split("abc", ',', false);
	assert_non_null(list);
	assert_string_equal(list[0], "abc");
	assert_null(list[1]);
	strlist_free(list);

	/* empty input */
	list = strlist_split("", ',', true);
	assert_non_null(list);
	assert_string_equal(list[0], "");
	assert_null(list[1]);
	strlist_free(list);

	list = strlist_split("", ',', false);
	assert_non_null(list);
	assert_null(list[0]);
	strlist_free(list);

	/* NULL input */
	assert_null(strlist_split(NULL, ',', true));
}

static void test_dstr_copy(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr d;
	dstr_init(&d);

	dstr_copy(&d, "hello");
	assert_string_equal(d.array, "hello");
	assert_int_equal(d.len, 5);
	assert_true(d.capacity > d.len);

	/* copy shorter over longer */
	dstr_copy(&d, "hi");
	assert_string_equal(d.array, "hi");
	assert_int_equal(d.len, 2);
	assert_true(d.capacity > d.len);

	/* copy longer */
	dstr_copy(&d, "a considerably longer string");
	assert_string_equal(d.array, "a considerably longer string");
	assert_int_equal(d.len, 28);
	assert_true(d.capacity > d.len);

	/* NULL and empty free the destination */
	dstr_copy(&d, NULL);
	assert_null(d.array);
	assert_int_equal(d.len, 0);
	assert_int_equal(d.capacity, 0);

	dstr_copy(&d, "x");
	assert_string_equal(d.array, "x");
	dstr_copy(&d, "");
	assert_null(d.array);
	assert_int_equal(d.len, 0);
	assert_int_equal(d.capacity, 0);

	dstr_free(&d);
}

static void test_dstr_ncopy(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr d, src;
	dstr_init(&d);
	dstr_init(&src);

	dstr_ncopy(&d, "hello world", 5);
	assert_string_equal(d.array, "hello");
	assert_int_equal(d.len, 5);
	assert_int_equal(d.capacity, 6);

	/* previous contents are replaced */
	dstr_ncopy(&d, "abcdef", 2);
	assert_string_equal(d.array, "ab");
	assert_int_equal(d.len, 2);
	assert_int_equal(d.capacity, 3);

	/* zero length frees */
	dstr_ncopy(&d, "abc", 0);
	assert_null(d.array);
	assert_int_equal(d.len, 0);
	assert_int_equal(d.capacity, 0);

	dstr_copy(&src, "abcdef");
	dstr_ncopy_dstr(&d, &src, 3);
	assert_string_equal(d.array, "abc");
	assert_int_equal(d.len, 3);
	assert_int_equal(d.capacity, 4);

	/* length is clamped to the source length */
	dstr_ncopy_dstr(&d, &src, 100);
	assert_string_equal(d.array, "abcdef");
	assert_int_equal(d.len, 6);
	assert_int_equal(d.capacity, 7);

	dstr_ncopy_dstr(&d, &src, 0);
	assert_null(d.array);
	assert_int_equal(d.len, 0);

	dstr_free(&d);
	dstr_free(&src);
}

static void test_dstr_cat(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr d, other;
	dstr_init(&d);
	dstr_init(&other);

	/* NULL / empty are no-ops and do not allocate */
	dstr_cat(&d, NULL);
	dstr_cat(&d, "");
	assert_null(d.array);
	assert_int_equal(d.len, 0);

	dstr_cat(&d, "foo");
	assert_string_equal(d.array, "foo");
	assert_int_equal(d.len, 3);
	assert_true(d.capacity > d.len);

	dstr_cat(&d, "bar");
	assert_string_equal(d.array, "foobar");
	assert_int_equal(d.len, 6);
	assert_true(d.capacity > d.len);

	dstr_cat_ch(&d, '!');
	assert_string_equal(d.array, "foobar!");
	assert_int_equal(d.len, 7);
	assert_true(d.capacity > d.len);

	dstr_ncat(&d, "123456", 3);
	assert_string_equal(d.array, "foobar!123");
	assert_int_equal(d.len, 10);
	assert_true(d.capacity > d.len);

	/* zero length / NULL / empty are no-ops */
	dstr_ncat(&d, "zzz", 0);
	dstr_ncat(&d, NULL, 3);
	dstr_ncat(&d, "", 3);
	assert_string_equal(d.array, "foobar!123");
	assert_int_equal(d.len, 10);

	dstr_copy(&other, "-tail");
	dstr_cat_dstr(&d, &other);
	assert_string_equal(d.array, "foobar!123-tail");
	assert_int_equal(d.len, 15);
	assert_true(d.capacity > d.len);

	/* empty source is a no-op */
	dstr_free(&other);
	dstr_cat_dstr(&d, &other);
	assert_string_equal(d.array, "foobar!123-tail");
	assert_int_equal(d.len, 15);

	dstr_copy(&other, "abcdef");
	dstr_ncat_dstr(&d, &other, 2);
	assert_string_equal(d.array, "foobar!123-tailab");
	assert_int_equal(d.len, 17);

	/* length is clamped to the source length */
	dstr_ncat_dstr(&d, &other, 100);
	assert_string_equal(d.array, "foobar!123-tailababcdef");
	assert_int_equal(d.len, 23);
	assert_true(d.capacity > d.len);

	dstr_free(&d);
	dstr_free(&other);
}

static void test_dstr_insert(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr d, other;
	dstr_init(&d);
	dstr_init(&other);

	/* inserting at len (0 here) is a cat */
	dstr_insert(&d, 0, "ad");
	assert_string_equal(d.array, "ad");
	assert_int_equal(d.len, 2);

	dstr_insert(&d, 1, "bc");
	assert_string_equal(d.array, "abcd");
	assert_int_equal(d.len, 4);
	assert_true(d.capacity > d.len);

	dstr_insert(&d, 4, "ef");
	assert_string_equal(d.array, "abcdef");
	assert_int_equal(d.len, 6);

	dstr_insert(&d, 0, "X");
	assert_string_equal(d.array, "Xabcdef");
	assert_int_equal(d.len, 7);
	assert_true(d.capacity > d.len);

	/* NULL / empty are no-ops */
	dstr_insert(&d, 2, NULL);
	dstr_insert(&d, 2, "");
	assert_string_equal(d.array, "Xabcdef");
	assert_int_equal(d.len, 7);

	dstr_copy(&other, "--");
	dstr_insert_dstr(&d, 3, &other);
	assert_string_equal(d.array, "Xab--cdef");
	assert_int_equal(d.len, 9);
	assert_true(d.capacity > d.len);

	dstr_insert_dstr(&d, 9, &other);
	assert_string_equal(d.array, "Xab--cdef--");
	assert_int_equal(d.len, 11);

	dstr_free(&other);
	dstr_insert_dstr(&d, 1, &other);
	assert_string_equal(d.array, "Xab--cdef--");
	assert_int_equal(d.len, 11);

	dstr_free(&d);
}

static void test_dstr_insert_ch(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr d;
	dstr_init(&d);

	/* idx == len is a cat_ch, including on an empty dstr */
	dstr_insert_ch(&d, 0, 'a');
	assert_string_equal(d.array, "a");
	assert_int_equal(d.len, 1);

	dstr_insert_ch(&d, 1, 'c');
	assert_string_equal(d.array, "ac");
	assert_int_equal(d.len, 2);

	/* A mid-string insert_ch moves one byte more than needed (it writes
	 * array[new_len + 1]), which is past the buffer when capacity is exactly
	 * new_len + 1. Reserve room so the test never relies on that
	 * out-of-bounds write (characterized, not endorsed). */
	dstr_reserve(&d, 16);

	dstr_insert_ch(&d, 1, 'b');
	assert_string_equal(d.array, "abc");
	assert_int_equal(d.len, 3);
	assert_true(d.capacity > d.len);

	dstr_insert_ch(&d, 0, 'X');
	assert_string_equal(d.array, "Xabc");
	assert_int_equal(d.len, 4);
	assert_true(d.capacity > d.len);

	dstr_insert_ch(&d, 4, 'Z');
	assert_string_equal(d.array, "XabcZ");
	assert_int_equal(d.len, 5);

	dstr_free(&d);
}

static void test_dstr_remove(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr d;
	dstr_init(&d);

	dstr_copy(&d, "abcdef");

	dstr_remove(&d, 1, 0);
	assert_string_equal(d.array, "abcdef");
	assert_int_equal(d.len, 6);

	/* middle */
	dstr_remove(&d, 1, 2);
	assert_string_equal(d.array, "adef");
	assert_int_equal(d.len, 4);
	assert_true(d.capacity > d.len);

	/* tail */
	dstr_remove(&d, 2, 2);
	assert_string_equal(d.array, "ad");
	assert_int_equal(d.len, 2);
	assert_true(d.capacity > d.len);

	/* head */
	dstr_remove(&d, 0, 1);
	assert_string_equal(d.array, "d");
	assert_int_equal(d.len, 1);

	/* removing everything frees */
	dstr_remove(&d, 0, 1);
	assert_null(d.array);
	assert_int_equal(d.len, 0);
	assert_int_equal(d.capacity, 0);

	dstr_free(&d);
}

static void test_dstr_replace(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr d;
	dstr_init(&d);

	/* empty dstr is a no-op */
	dstr_replace(&d, "a", "b");
	assert_null(d.array);
	assert_int_equal(d.len, 0);

	/* replacement shorter than find */
	dstr_copy(&d, "foo bar foo");
	dstr_replace(&d, "foo", "x");
	assert_string_equal(d.array, "x bar x");
	assert_int_equal(d.len, 7);
	assert_true(d.capacity > d.len);

	/* empty replacement */
	dstr_copy(&d, "foo bar foo");
	dstr_replace(&d, "foo", "");
	assert_string_equal(d.array, " bar ");
	assert_int_equal(d.len, 5);

	/* NULL replacement behaves like empty */
	dstr_copy(&d, "foo bar foo");
	dstr_replace(&d, "foo", NULL);
	assert_string_equal(d.array, " bar ");
	assert_int_equal(d.len, 5);

	/* replacement longer than find */
	dstr_copy(&d, "a-b-c");
	dstr_replace(&d, "-", "--");
	assert_string_equal(d.array, "a--b--c");
	assert_int_equal(d.len, 7);
	assert_true(d.capacity > d.len);

	dstr_copy(&d, "-start and end-");
	dstr_replace(&d, "-", "<longer>");
	assert_string_equal(d.array, "<longer>start and end<longer>");
	assert_int_equal(d.len, 29);
	assert_true(d.capacity > d.len);

	/* replacement same length as find */
	dstr_copy(&d, "abcabc");
	dstr_replace(&d, "abc", "xyz");
	assert_string_equal(d.array, "xyzxyz");
	assert_int_equal(d.len, 6);

	/* no match, every branch */
	dstr_copy(&d, "hello");
	dstr_replace(&d, "zzz", "y");
	assert_string_equal(d.array, "hello");
	assert_int_equal(d.len, 5);
	dstr_replace(&d, "zzz", "yyyyyy");
	assert_string_equal(d.array, "hello");
	assert_int_equal(d.len, 5);
	dstr_replace(&d, "zzz", "yyy");
	assert_string_equal(d.array, "hello");
	assert_int_equal(d.len, 5);

	/* an empty find string is deliberately not tested: the C code loops
	 * forever because strstr(temp, "") never advances temp. */

	dstr_free(&d);
}

static void test_dstr_depad(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr d;
	dstr_init(&d);

	/* no array is a no-op */
	dstr_depad(&d);
	assert_null(d.array);
	assert_int_equal(d.len, 0);

	dstr_copy(&d, "  \thi there \r\n");
	dstr_depad(&d);
	assert_string_equal(d.array, "hi there");
	assert_int_equal(d.len, 8);
	assert_true(d.capacity > d.len);

	dstr_copy(&d, "tight");
	dstr_depad(&d);
	assert_string_equal(d.array, "tight");
	assert_int_equal(d.len, 5);

	/* all whitespace frees */
	dstr_copy(&d, " \t\r\n ");
	dstr_depad(&d);
	assert_null(d.array);
	assert_int_equal(d.len, 0);
	assert_int_equal(d.capacity, 0);

	dstr_free(&d);
}

static void test_dstr_left_mid_right(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr src, d;
	dstr_init(&src);
	dstr_init(&d);

	dstr_copy(&src, "hello world");

	dstr_left(&d, &src, 5);
	assert_string_equal(d.array, "hello");
	assert_int_equal(d.len, 5);
	assert_true(d.capacity > d.len);

	/* in-place (dst == src) */
	dstr_left(&src, &src, 8);
	assert_string_equal(src.array, "hello wo");
	assert_int_equal(src.len, 8);
	assert_true(src.capacity > src.len);

	dstr_copy(&src, "hello world");

	dstr_mid(&d, &src, 6, 5);
	assert_string_equal(d.array, "world");
	assert_int_equal(d.len, 5);
	assert_true(d.capacity > d.len);

	dstr_mid(&d, &src, 2, 3);
	assert_string_equal(d.array, "llo");
	assert_int_equal(d.len, 3);

	/* zero count frees dst */
	dstr_mid(&d, &src, 2, 0);
	assert_null(d.array);
	assert_int_equal(d.len, 0);

	dstr_right(&d, &src, 6);
	assert_string_equal(d.array, "world");
	assert_int_equal(d.len, 5);
	assert_true(d.capacity > d.len);

	dstr_right(&d, &src, 0);
	assert_string_equal(d.array, "hello world");
	assert_int_equal(d.len, 11);

	/* pos == len leaves dst empty */
	dstr_right(&d, &src, 11);
	assert_null(d.array);
	assert_int_equal(d.len, 0);

	dstr_free(&d);
	dstr_free(&src);
}

#ifndef _WIN32
static void test_dstr_case(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr d;
	dstr_init(&d);

	/* empty is a no-op */
	dstr_to_upper(&d);
	dstr_to_lower(&d);
	assert_null(d.array);

	dstr_copy(&d, "Hello, World 123");
	dstr_to_upper(&d);
	assert_string_equal(d.array, "HELLO, WORLD 123");
	assert_int_equal(d.len, 16);
	assert_true(d.capacity > d.len);

	dstr_to_lower(&d);
	assert_string_equal(d.array, "hello, world 123");
	assert_int_equal(d.len, 16);
	assert_true(d.capacity > d.len);

	dstr_free(&d);
}
#endif

static void test_dstr_printf(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr d;
	dstr_init(&d);

	dstr_printf(&d, "%d-%s", 42, "abc");
	assert_string_equal(d.array, "42-abc");
	assert_int_equal(d.len, 6);
	assert_true(d.capacity > d.len);

	/* overwrites previous contents */
	dstr_printf(&d, "%d-%s", 7, "x");
	assert_string_equal(d.array, "7-x");
	assert_int_equal(d.len, 3);
	assert_true(d.capacity > d.len);

	dstr_catf(&d, " [%03d]", 7);
	assert_string_equal(d.array, "7-x [007]");
	assert_int_equal(d.len, 9);
	assert_true(d.capacity > d.len);

	/* printf producing an empty string frees */
	dstr_printf(&d, "%s", "");
	assert_null(d.array);
	assert_int_equal(d.len, 0);
	assert_int_equal(d.capacity, 0);

	/* catf onto a fresh dstr */
	dstr_catf(&d, "%s=%d", "k", 1);
	assert_string_equal(d.array, "k=1");
	assert_int_equal(d.len, 3);
	assert_true(d.capacity > d.len);

	dstr_free(&d);
}

static void test_dstr_safe_printf(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr d;
	dstr_init(&d);

	dstr_safe_printf(&d, "$1 and $2 and $3 and $4", "a", "bb", "ccc", "dddd");
	assert_string_equal(d.array, "a and bb and ccc and dddd");
	assert_int_equal(d.len, 25);
	assert_true(d.capacity > d.len);

	/* NULL values leave their placeholder untouched */
	dstr_safe_printf(&d, "x=$1 y=$2", "1", NULL, NULL, NULL);
	assert_string_equal(d.array, "x=1 y=$2");
	assert_int_equal(d.len, 8);

	/* repeated placeholder */
	dstr_safe_printf(&d, "$1$1", "ab", NULL, NULL, NULL);
	assert_string_equal(d.array, "abab");
	assert_int_equal(d.len, 4);

	/* empty value removes the placeholder */
	dstr_safe_printf(&d, "[$1]", "", NULL, NULL, NULL);
	assert_string_equal(d.array, "[]");
	assert_int_equal(d.len, 2);

	/* NULL format frees */
	dstr_safe_printf(&d, NULL, "a", "b", "c", "d");
	assert_null(d.array);
	assert_int_equal(d.len, 0);

	dstr_free(&d);
}

static void test_dstr_strref(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr d;
	struct strref ref;
	dstr_init(&d);

	strref_set(&ref, "hello world", 5);
	dstr_copy_strref(&d, &ref);
	assert_string_equal(d.array, "hello");
	assert_int_equal(d.len, 5);
	assert_int_equal(d.capacity, 6);

	/* replaces previous contents */
	strref_set(&ref, "abcdef", 3);
	dstr_copy_strref(&d, &ref);
	assert_string_equal(d.array, "abc");
	assert_int_equal(d.len, 3);

	dstr_cat_strref(&d, &ref);
	assert_string_equal(d.array, "abcabc");
	assert_int_equal(d.len, 6);
	assert_true(d.capacity > d.len);

	/* empty strref */
	strref_set(&ref, "xyz", 0);
	dstr_cat_strref(&d, &ref);
	assert_string_equal(d.array, "abcabc");
	assert_int_equal(d.len, 6);

	dstr_copy_strref(&d, &ref);
	assert_null(d.array);
	assert_int_equal(d.len, 0);

	dstr_free(&d);

	strref_set(&ref, "init-copy", 4);
	dstr_init_copy_strref(&d, &ref);
	assert_string_equal(d.array, "init");
	assert_int_equal(d.len, 4);
	assert_int_equal(d.capacity, 5);

	dstr_free(&d);
}

static void test_dstr_inline_helpers(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr d;
	dstr_init(&d);

	assert_true(dstr_is_empty(&d));
	assert_int_equal(dstr_end(&d), 0);
	assert_int_equal(dstr_cmp(&d, NULL), 0);
	assert_int_equal(dstr_cmp(&d, ""), 0);

	dstr_copy(&d, "Hello");
	assert_false(dstr_is_empty(&d));
	assert_int_equal(dstr_end(&d), 'o');
	assert_int_equal(dstr_cmp(&d, "Hello"), 0);
	assert_true(dstr_cmp(&d, "hello") < 0);
	assert_int_equal(dstr_cmpi(&d, "hELLO"), 0);
	assert_int_equal(dstr_ncmp(&d, "Help", 3), 0);
	assert_int_equal(dstr_ncmpi(&d, "HELP", 3), 0);
	assert_ptr_equal(dstr_find(&d, "llo"), d.array + 2);
	assert_ptr_equal(dstr_find_i(&d, "LLO"), d.array + 2);
	assert_null(dstr_find(&d, "LLO"));

	dstr_free(&d);
}

/* ------------------------------------------------------------------------
 * util/dstr-libc.c: the wide-character helpers, the printf family and the
 * multibyte and wide conversions. They call the C library (towupper,
 * towlower, vsnprintf, mbstowcs), so some results depend on the libc and the
 * current locale. Those cases say which, and are skipped where the locale is
 * missing. */

static void test_wstrcmpi(void **state)
{
	UNUSED_PARAMETER(state);

	assert_int_equal(wstrcmpi(L"abc", L"abc"), 0);
	assert_int_equal(wstrcmpi(L"abc", L"ABC"), 0);
	assert_int_equal(wstrcmpi(L"aBc", L"AbC"), 0);
	assert_int_equal(wstrcmpi(L"a", L"b"), -1);
	assert_int_equal(wstrcmpi(L"B", L"a"), 1);
	assert_int_equal(wstrcmpi(L"ab", L"abc"), -1);
	assert_int_equal(wstrcmpi(L"abc", L"ab"), 1);
	assert_int_equal(wstrcmpi(L"", L""), 0);

	/* both sides are compared upper-cased: '[' (0x5b) sorts after 'A' */
	assert_int_equal(wstrcmpi(L"[", L"a"), 1);
	assert_int_equal(wstrcmpi(L"a", L"["), -1);

	/* NULL is treated as the empty string */
	assert_int_equal(wstrcmpi(NULL, NULL), 0);
	assert_int_equal(wstrcmpi(NULL, L""), 0);
	assert_int_equal(wstrcmpi(L"", NULL), 0);
	assert_int_equal(wstrcmpi(NULL, L"a"), -1);
	assert_int_equal(wstrcmpi(L"a", NULL), 1);
}

static void test_wstrcmp_n(void **state)
{
	UNUSED_PARAMETER(state);

	assert_int_equal(wstrcmp_n(L"abc", L"abd", 2), 0);
	assert_int_equal(wstrcmp_n(L"abc", L"abd", 3), -1);
	assert_int_equal(wstrcmp_n(L"abd", L"abc", 3), 1);
	assert_int_equal(wstrcmp_n(L"abc", L"abc", 10), 0);
	assert_int_equal(wstrcmp_n(L"ab", L"abc", 5), -1);
	assert_int_equal(wstrcmp_n(L"abc", L"ab", 5), 1);

	/* case-sensitive: 'a' (0x61) > 'A' (0x41) */
	assert_int_equal(wstrcmp_n(L"a", L"A", 1), 1);
	assert_int_equal(wstrcmp_n(L"A", L"a", 1), -1);

	/* n == 0 compares nothing, even NULL against a string */
	assert_int_equal(wstrcmp_n(L"a", L"b", 0), 0);
	assert_int_equal(wstrcmp_n(NULL, L"b", 0), 0);

	/* NULL is treated as the empty string */
	assert_int_equal(wstrcmp_n(NULL, NULL, 4), 0);
	assert_int_equal(wstrcmp_n(NULL, L"", 4), 0);
	assert_int_equal(wstrcmp_n(NULL, L"x", 1), -1);
	assert_int_equal(wstrcmp_n(L"x", NULL, 1), 1);
}

static void test_wstrcmpi_n(void **state)
{
	UNUSED_PARAMETER(state);

	assert_int_equal(wstrcmpi_n(L"abc", L"ABD", 2), 0);
	assert_int_equal(wstrcmpi_n(L"abc", L"ABD", 3), -1);
	assert_int_equal(wstrcmpi_n(L"ABD", L"abc", 3), 1);
	assert_int_equal(wstrcmpi_n(L"Hello", L"hELLO", 100), 0);
	assert_int_equal(wstrcmpi_n(L"ab", L"ABC", 5), -1);
	assert_int_equal(wstrcmpi_n(L"[", L"a", 1), 1);

	assert_int_equal(wstrcmpi_n(L"a", L"b", 0), 0);
	assert_int_equal(wstrcmpi_n(NULL, NULL, 3), 0);
	assert_int_equal(wstrcmpi_n(NULL, L"a", 1), -1);
	assert_int_equal(wstrcmpi_n(L"a", NULL, 1), 1);
}

static void test_wstrstri(void **state)
{
	UNUSED_PARAMETER(state);

	const wchar_t *s = L"Hello World";

	assert_ptr_equal(wstrstri(s, L"world"), s + 6);
	assert_ptr_equal(wstrstri(s, L"HELLO"), s);
	assert_ptr_equal(wstrstri(s, L"d"), s + 10);
	assert_ptr_equal(wstrstri(s, L"lo w"), s + 3);
	assert_null(wstrstri(s, L"xyz"));
	assert_null(wstrstri(s, L"Hello World!"));

	/* the first match wins */
	const wchar_t *rep = L"xABab";
	assert_ptr_equal(wstrstri(rep, L"ab"), rep + 1);

	/* an empty find matches at the start, also of an empty string */
	assert_ptr_equal(wstrstri(s, L""), s);
	const wchar_t *empty = L"";
	assert_ptr_equal(wstrstri(empty, L""), empty);
	assert_null(wstrstri(empty, L"a"));

	/* NULL on either side returns NULL */
	assert_null(wstrstri(NULL, L"a"));
	assert_null(wstrstri(s, NULL));
	assert_null(wstrstri(NULL, NULL));
}

static void test_wcsdepad(void **state)
{
	UNUSED_PARAMETER(state);

	wchar_t buf[64];

	wcscpy(buf, L"  \t hi there \r\n");
	assert_ptr_equal(wcsdepad(buf), buf);
	assert_true(wcscmp(buf, L"hi there") == 0);

	wcscpy(buf, L"nopad");
	assert_ptr_equal(wcsdepad(buf), buf);
	assert_true(wcscmp(buf, L"nopad") == 0);

	wcscpy(buf, L"   lead");
	wcsdepad(buf);
	assert_true(wcscmp(buf, L"lead") == 0);

	wcscpy(buf, L"trail \t\n");
	wcsdepad(buf);
	assert_true(wcscmp(buf, L"trail") == 0);

	/* only ' ', '\t', '\n' and '\r' are padding; '\v' and '\f' stay */
	wcscpy(buf, L"\v x \f");
	wcsdepad(buf);
	assert_true(wcscmp(buf, L"\v x \f") == 0);

	/* all padding leaves an empty string */
	wcscpy(buf, L" \t\r\n ");
	assert_ptr_equal(wcsdepad(buf), buf);
	assert_true(wcscmp(buf, L"") == 0);

	wcscpy(buf, L"");
	assert_ptr_equal(wcsdepad(buf), buf);
	assert_true(wcscmp(buf, L"") == 0);

	/* the leading padding is moved out with memmove (string and NUL); the
	 * trailing padding is overwritten with NULs one by one, so the old tail
	 * past them is left as it was. Characterized, not endorsed. */
	wmemset(buf, L'#', 8);
	wcscpy(buf, L"  ab  ");
	wcsdepad(buf);
	assert_true(wcscmp(buf, L"ab") == 0);
	assert_int_equal(buf[2], 0);
	assert_int_equal(buf[3], 0);
	assert_int_equal(buf[4], 0);
	assert_int_equal(buf[5], L' ');
	assert_int_equal(buf[6], 0);
	assert_int_equal(buf[7], L'#');

	assert_null(wcsdepad(NULL));
}

/* printf attribute for the va_list wrappers below; dstr.h #undefs its own. */
#ifndef _MSC_VER
#define TEST_PRINTFATTR(f, a) __attribute__((__format__(__printf__, f, a)))
#else
#define TEST_PRINTFATTR(f, a)
#endif

TEST_PRINTFATTR(2, 3)
static void call_vprintf(struct dstr *d, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	dstr_vprintf(d, format, args);
	va_end(args);
}

TEST_PRINTFATTR(2, 3)
static void call_vcatf(struct dstr *d, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	dstr_vcatf(d, format, args);
	va_end(args);
}

static void test_dstr_printf_formats(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr d;
	dstr_init(&d);

	/* the buffer is sized exactly: capacity = len + 1 on a fresh dstr */
	dstr_printf(&d, "%s|%d|%u|%zu|%x|%c|%%|%5.2f|%-4s|%.*s", "s", -3, 7u, (size_t)42, 255u, 'z', 1.5, "ab", 2,
		    "xyz");
	assert_string_equal(d.array, "s|-3|7|42|ff|z|%| 1.50|ab  |xy");
	assert_int_equal(d.len, 30);
	assert_int_equal(d.capacity, 31);

	/* a shorter printf keeps the bigger buffer */
	dstr_printf(&d, "%d", 5);
	assert_string_equal(d.array, "5");
	assert_int_equal(d.len, 1);
	assert_int_equal(d.capacity, 31);

	/* catf grows with dstr_ensure_capacity: double, or the size if larger */
	dstr_printf(&d, "%s", "0123456789012345678901234567");
	assert_int_equal(d.len, 28);
	assert_int_equal(d.capacity, 31);
	dstr_catf(&d, "%s", "abcd");
	assert_string_equal(d.array, "0123456789012345678901234567abcd");
	assert_int_equal(d.len, 32);
	assert_int_equal(d.capacity, 62);

	/* catf of an empty result onto a non-empty string keeps it */
	dstr_catf(&d, "%s", "");
	assert_int_equal(d.len, 32);
	assert_int_equal(d.capacity, 62);
	dstr_free(&d);

	/* catf of an empty result onto an empty dstr allocates and then frees */
	dstr_catf(&d, "%s", "");
	assert_null(d.array);
	assert_int_equal(d.len, 0);
	assert_int_equal(d.capacity, 0);

	/* a result whose first byte is NUL frees, even though vsnprintf
	 * reported a length. Characterized, not endorsed. */
	dstr_printf(&d, "%c", 0);
	assert_null(d.array);
	assert_int_equal(d.len, 0);
	assert_int_equal(d.capacity, 0);

	/* an embedded NUL later on is counted in len (vsnprintf's count) */
	dstr_printf(&d, "a%cb", 0);
	assert_int_equal(d.len, 3);
	assert_int_equal(d.capacity, 4);
	assert_int_equal(d.array[0], 'a');
	assert_int_equal(d.array[1], 0);
	assert_int_equal(d.array[2], 'b');
	assert_int_equal(d.array[3], 0);

	/* catf appends after len, past the embedded NUL */
	dstr_catf(&d, "%s", "c");
	assert_int_equal(d.len, 4);
	assert_int_equal(d.array[3], 'c');

	dstr_free(&d);
}

static void test_dstr_vprintf(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr d;
	dstr_init(&d);

	call_vprintf(&d, "%s-%d", "va", 1);
	assert_string_equal(d.array, "va-1");
	assert_int_equal(d.len, 4);
	assert_int_equal(d.capacity, 5);

	call_vcatf(&d, "+%s", "list");
	assert_string_equal(d.array, "va-1+list");
	assert_int_equal(d.len, 9);
	assert_int_equal(d.capacity, 10);

	call_vprintf(&d, "%s", "");
	assert_null(d.array);
	assert_int_equal(d.len, 0);
	assert_int_equal(d.capacity, 0);

	call_vcatf(&d, "%d", 12);
	assert_string_equal(d.array, "12");
	assert_int_equal(d.len, 2);
	assert_int_equal(d.capacity, 3);

	dstr_free(&d);
}

static void test_dstr_printf_long(void **state)
{
	UNUSED_PARAMETER(state);

	const size_t n = 100000;
	char *big = bmalloc(n + 1);
	memset(big, 'q', n);
	big[n] = 0;

	struct dstr d;
	dstr_init(&d);

	dstr_printf(&d, "%s", big);
	assert_int_equal(d.len, n);
	assert_int_equal(d.capacity, n + 1);
	assert_int_equal(d.array[n - 1], 'q');
	assert_int_equal(d.array[n], 0);

	dstr_catf(&d, "%s!", big);
	assert_int_equal(d.len, 2 * n + 1);
	assert_int_equal(d.capacity, 2 * n + 2);
	assert_int_equal(d.array[2 * n], '!');
	assert_int_equal(d.array[2 * n + 1], 0);

	/* 4095 and 4096 bytes, around the length the error path assumes */
	memset(big, 'r', 4096);
	big[4095] = 0;
	dstr_free(&d);
	dstr_printf(&d, "%s", big);
	assert_int_equal(d.len, 4095);
	assert_int_equal(d.capacity, 4096);
	big[4095] = 'r';
	big[4096] = 0;
	dstr_printf(&d, "%s", big);
	assert_int_equal(d.len, 4096);
	assert_int_equal(d.capacity, 8192);

	dstr_free(&d);
	bfree(big);
}

#ifdef __GLIBC__
/* glibc: in the "C" locale, %ls of a non-ASCII wide character fails
 * (EILSEQ) and vsnprintf returns -1. The C then assumes 4095 bytes, sizes the
 * buffer to 4096, and takes the length with strlen of what glibc wrote (the
 * output up to the failing conversion). Characterized, not endorsed. */
static void test_dstr_printf_error_path(void **state)
{
	UNUSED_PARAMETER(state);

	char *saved = bstrdup(setlocale(LC_ALL, NULL));
	setlocale(LC_ALL, "C");

	struct dstr d;
	dstr_init(&d);

	dstr_printf(&d, "ab%lscd", L"é");
	assert_string_equal(d.array, "ab");
	assert_int_equal(d.len, 2);
	assert_int_equal(d.capacity, 4096);

	dstr_copy(&d, "x");
	dstr_catf(&d, "ab%lscd", L"é");
	assert_string_equal(d.array, "xab");
	assert_int_equal(d.len, 3);
	assert_int_equal(d.capacity, 8192);

	/* a failure before any output leaves an empty string, which frees */
	dstr_printf(&d, "%ls", L"é");
	assert_null(d.array);
	assert_int_equal(d.len, 0);
	assert_int_equal(d.capacity, 0);

	dstr_free(&d);
	setlocale(LC_ALL, saved);
	bfree(saved);
}
#endif

static void test_dstr_mbs(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr d;
	dstr_init(&d);

	/* from_mbs moves the converted buffer in: capacity stays 0.
	 * Characterized, not endorsed. */
	dstr_from_mbs(&d, "hello");
	assert_string_equal(d.array, "hello");
	assert_int_equal(d.len, 5);
	assert_int_equal(d.capacity, 0);

	/* an empty string gives an allocated "" */
	dstr_from_mbs(&d, "");
	assert_non_null(d.array);
	assert_string_equal(d.array, "");
	assert_int_equal(d.len, 0);
	assert_int_equal(d.capacity, 0);

	/* NULL frees */
	dstr_from_mbs(&d, NULL);
	assert_null(d.array);
	assert_int_equal(d.len, 0);
	assert_int_equal(d.capacity, 0);

	/* to_mbs converts multibyte to UTF-8 (os_mbs_to_utf8_ptr), not the
	 * other way round, and stops at the first NUL. */
	char *m = dstr_to_mbs(&d);
	assert_null(m);
	dstr_copy(&d, "abc");
	m = dstr_to_mbs(&d);
	assert_string_equal(m, "abc");
	bfree(m);
	dstr_ncopy(&d, "a\0b", 3);
	assert_int_equal(d.len, 3);
	m = dstr_to_mbs(&d);
	assert_string_equal(m, "a");
	bfree(m);

	dstr_free(&d);
}

static void test_dstr_wcs(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr d;
	dstr_init(&d);

	/* to_wcs of an unallocated dstr is NULL */
	assert_null(dstr_to_wcs(&d));

	dstr_copy(&d, "Hello");
	wchar_t *w = dstr_to_wcs(&d);
	assert_true(wcscmp(w, L"Hello") == 0);
	bfree(w);

	/* an allocated "" gives an allocated L"" */
	dstr_from_mbs(&d, "");
	w = dstr_to_wcs(&d);
	assert_non_null(w);
	assert_int_equal(w[0], 0);
	bfree(w);

	/* from_wcs sizes the buffer to the UTF-8 length + 1 */
	dstr_free(&d);
	dstr_from_wcs(&d, L"wide");
	assert_string_equal(d.array, "wide");
	assert_int_equal(d.len, 4);
	assert_int_equal(d.capacity, 5);

	/* into a bigger buffer it keeps the capacity */
	dstr_copy(&d, "0123456789");
	dstr_from_wcs(&d, L"ab");
	assert_string_equal(d.array, "ab");
	assert_int_equal(d.len, 2);
	assert_int_equal(d.capacity, 11);

	/* L"" and NULL free */
	dstr_from_wcs(&d, L"");
	assert_null(d.array);
	assert_int_equal(d.len, 0);
	assert_int_equal(d.capacity, 0);
	dstr_copy(&d, "x");
	dstr_from_wcs(&d, NULL);
	assert_null(d.array);
	assert_int_equal(d.capacity, 0);

	dstr_free(&d);
}

#ifndef _WIN32
static void test_dstr_wcs_utf8(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr d;
	dstr_init(&d);

	/* 2-, 3- and 4-byte UTF-8 round trip through wchar_t */
	const char *text = "h\xc3\xa9llo \xe2\x82\xac \xf0\x9f\x98\x80";
	dstr_copy(&d, text);
	wchar_t *w = dstr_to_wcs(&d);
	assert_true(wcscmp(w, L"héllo € \U0001F600") == 0);
	dstr_free(&d);
	dstr_from_wcs(&d, w);
	assert_string_equal(d.array, text);
	assert_int_equal(d.len, strlen(text));
	assert_int_equal(d.capacity, strlen(text) + 1);
	bfree(w);

	/* to_wcs converts len bytes, so an embedded NUL is converted too */
	dstr_ncopy(&d, "a\0b", 3);
	w = dstr_to_wcs(&d);
	assert_int_equal(w[0], L'a');
	assert_int_equal(w[1], 0);
	assert_int_equal(w[2], L'b');
	assert_int_equal(w[3], 0);
	bfree(w);

	/* invalid UTF-8 converts to L"" */
	dstr_copy(&d, "ok\xff");
	w = dstr_to_wcs(&d);
	assert_non_null(w);
	assert_int_equal(w[0], 0);
	bfree(w);

	/* a surrogate code point has no UTF-8 form: from_wcs frees */
	wchar_t sur[] = {L'a', (wchar_t)0xd800, 0};
	dstr_copy(&d, "x");
	dstr_from_wcs(&d, sur);
	assert_null(d.array);
	assert_int_equal(d.len, 0);
	assert_int_equal(d.capacity, 0);

	dstr_free(&d);
}

static void test_dstr_case_edges(void **state)
{
	UNUSED_PARAMETER(state);

	struct dstr d;
	dstr_init(&d);

	/* an allocated "" is empty: no-op */
	dstr_from_mbs(&d, "");
	char *before = d.array;
	dstr_to_upper(&d);
	assert_ptr_equal(d.array, before);
	assert_int_equal(d.len, 0);

	/* the case is mapped up to the first NUL; the string is rebuilt from
	 * there, so an embedded NUL truncates it. Characterized, not endorsed. */
	dstr_ncopy(&d, "a\0b", 3);
	dstr_to_upper(&d);
	assert_string_equal(d.array, "A");
	assert_int_equal(d.len, 1);

	/* invalid UTF-8 converts to L"", so the string is freed.
	 * Characterized, not endorsed. */
	dstr_copy(&d, "abc\xff");
	dstr_to_lower(&d);
	assert_null(d.array);
	assert_int_equal(d.len, 0);
	assert_int_equal(d.capacity, 0);

	/* a long string; the capacity is the UTF-8 length + 1 or the old one */
	const size_t n = 10000;
	char *big = bmalloc(n + 1);
	memset(big, 'm', n);
	big[n] = 0;
	dstr_copy(&d, big);
	dstr_to_upper(&d);
	assert_int_equal(d.len, n);
	assert_int_equal(d.capacity, n + 1);
	assert_int_equal(d.array[0], 'M');
	assert_int_equal(d.array[n - 1], 'M');
	dstr_to_lower(&d);
	assert_int_equal(d.array[n - 1], 'm');
	bfree(big);

	dstr_free(&d);
}

/* Switches LC_CTYPE (and with it towupper, towlower and mbstowcs) to a
 * UTF-8 locale. Returns the old one (bfree it), or NULL if none exists. */
static char *use_utf8_ctype(void)
{
	static const char *const names[] = {"C.UTF-8", "C.utf8", "en_US.UTF-8", "en_US.utf8"};
	char *saved = bstrdup(setlocale(LC_CTYPE, NULL));

	for (size_t i = 0; i < sizeof(names) / sizeof(names[0]); i++) {
		if (setlocale(LC_CTYPE, names[i]))
			return saved;
	}
	bfree(saved);
	return NULL;
}

static void test_dstr_locale(void **state)
{
	UNUSED_PARAMETER(state);

	char *saved = use_utf8_ctype();
	if (!saved) {
		skip();
		return;
	}

	struct dstr d;
	dstr_init(&d);

	/* the case comes from towupper / towlower of the current locale, one
	 * code point at a time */
	if (towupper(0xe9) == 0xc9 && towlower(0xc0) == 0xe0) {
		dstr_copy(&d, "h\xc3\xa9llo");
		dstr_to_upper(&d);
		assert_string_equal(d.array, "H\xc3\x89LLO");
		assert_int_equal(d.len, 6);
		dstr_copy(&d, "\xc3\x80\xc3\x89");
		dstr_to_lower(&d);
		assert_string_equal(d.array, "\xc3\xa0\xc3\xa9");
		assert_int_equal(d.len, 4);
	}

	/* one-to-one mapping: U+00DF has no single upper case, so it stays
	 * (no "SS") */
	if (towupper(0xdf) == 0xdf) {
		dstr_copy(&d, "stra\303\237e");
		dstr_to_upper(&d);
		assert_string_equal(d.array, "STRA\303\237E");
		assert_int_equal(d.len, 7);
	}

	/* the wide compares use towupper too */
	if (towupper(0xe9) == 0xc9) {
		assert_int_equal(wstrcmpi(L"été", L"ÉTÉ"), 0);
		assert_int_equal(wstrcmpi_n(L"éx", L"Éy", 1), 0);
		const wchar_t *s = L"café";
		assert_ptr_equal(wstrstri(s, L"É"), s + 3);
	}

	/* multibyte input in a UTF-8 locale is UTF-8 */
	dstr_from_mbs(&d, "h\xc3\xa9");
	assert_string_equal(d.array, "h\xc3\xa9");
	assert_int_equal(d.len, 3);
	assert_int_equal(d.capacity, 0);
	char *m = dstr_to_mbs(&d);
	assert_string_equal(m, "h\xc3\xa9");
	bfree(m);

	/* text the locale cannot convert gives an allocated "" */
	dstr_from_mbs(&d, "a\xff");
	assert_non_null(d.array);
	assert_string_equal(d.array, "");
	assert_int_equal(d.len, 0);

	dstr_free(&d);
	setlocale(LC_CTYPE, saved);
	bfree(saved);
}
#endif

#ifdef __GLIBC__
/* glibc's "C" locale is ASCII: towupper leaves U+00E9 alone, and mbstowcs
 * fails on a byte >= 0x80, which gives an allocated "". */
static void test_dstr_c_locale(void **state)
{
	UNUSED_PARAMETER(state);

	char *saved = bstrdup(setlocale(LC_CTYPE, NULL));
	setlocale(LC_CTYPE, "C");

	struct dstr d;
	dstr_init(&d);

	dstr_copy(&d, "h\xc3\xa9llo");
	dstr_to_upper(&d);
	assert_string_equal(d.array, "H\xc3\xa9LLO");

	assert_int_equal(wstrcmpi(L"é", L"É"), 1);

	dstr_from_mbs(&d, "h\xc3\xa9");
	assert_non_null(d.array);
	assert_string_equal(d.array, "");
	assert_int_equal(d.len, 0);

	dstr_free(&d);
	setlocale(LC_CTYPE, saved);
	bfree(saved);
}
#endif

int main()
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(test_astrcmpi, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_astrcmp_n, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_astrcmpi_n, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_astrstri, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_strdepad, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_strlist_split, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_dstr_copy, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_dstr_ncopy, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_dstr_cat, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_dstr_insert, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_dstr_insert_ch, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_dstr_remove, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_dstr_replace, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_dstr_depad, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_dstr_left_mid_right, leak_setup, leak_teardown),
#ifndef _WIN32
		cmocka_unit_test_setup_teardown(test_dstr_case, leak_setup, leak_teardown),
#endif
		cmocka_unit_test_setup_teardown(test_dstr_printf, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_dstr_safe_printf, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_dstr_strref, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_dstr_inline_helpers, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_wstrcmpi, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_wstrcmp_n, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_wstrcmpi_n, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_wstrstri, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_wcsdepad, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_dstr_printf_formats, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_dstr_vprintf, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_dstr_printf_long, leak_setup, leak_teardown),
#ifdef __GLIBC__
		cmocka_unit_test_setup_teardown(test_dstr_printf_error_path, leak_setup, leak_teardown),
#endif
		cmocka_unit_test_setup_teardown(test_dstr_mbs, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_dstr_wcs, leak_setup, leak_teardown),
#ifndef _WIN32
		cmocka_unit_test_setup_teardown(test_dstr_wcs_utf8, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_dstr_case_edges, leak_setup, leak_teardown),
		cmocka_unit_test_setup_teardown(test_dstr_locale, leak_setup, leak_teardown),
#endif
#ifdef __GLIBC__
		cmocka_unit_test_setup_teardown(test_dstr_c_locale, leak_setup, leak_teardown),
#endif
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
