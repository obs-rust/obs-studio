#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-internal.h"

/*
 * Characterization of the output protocol-list match in
 * libobs/obs-output-protocols.c, before its Rust port:
 * obs_output_protocol_list_count(protocols, protocol) counts the
 * ';'-separated segments of a service output's protocols string that equal
 * protocol byte for byte. obs_enum_output_types_with_protocol() calls enum_cb
 * that many times for the output. They pin the current C behavior; cases
 * marked "characterized, not endorsed" are odd but real.
 *
 * The function is hidden in libobs, so this test compiles the C source into
 * itself and calls only it, so the port can run the same file against the
 * Rust staticlib. test_output_protocols covers the caller through the real
 * libobs.
 *
 * The lists come from output plugins and the protocol from the stream
 * service (services.json, or derived from a custom server URL), so both are
 * any bytes but NUL. protocol is never NULL: the caller has checked that it
 * is registered.
 */

#define count obs_output_protocol_list_count

static void null_and_empty_list(void **state)
{
	(void)state;
	assert_int_equal(count(NULL, "RTMP"), 0);
	assert_int_equal(count("", "RTMP"), 0);
	assert_int_equal(count(NULL, ""), 0);
	assert_int_equal(count("", ""), 0);
}

static void whole_segments(void **state)
{
	(void)state;
	assert_int_equal(count("RTMP", "RTMP"), 1);
	assert_int_equal(count("RTMP;RTMPS", "RTMP"), 1);
	assert_int_equal(count("RTMP;RTMPS", "RTMPS"), 1);
	assert_int_equal(count("RTMPS;RTMP", "RTMP"), 1);
	assert_int_equal(count("SRT;RIST", "RIST"), 1);
	/* prefixes either way do not match */
	assert_int_equal(count("RTMPX", "RTMP"), 0);
	assert_int_equal(count("RTMP", "RTMPX"), 0);
	assert_int_equal(count("RTMP", "RTM"), 0);
	assert_int_equal(count("XRTMP", "RTMP"), 0);
	/* the separator is not part of a segment */
	assert_int_equal(count("RTMP;RTMPS", "RTMP;RTMPS"), 0);
	assert_int_equal(count("RTMP;", "RTMP;"), 0);
	assert_int_equal(count("A;B", "A;"), 0);
}

static void case_and_spaces(void **state)
{
	(void)state;
	assert_int_equal(count("rtmp", "RTMP"), 0);
	assert_int_equal(count("RTMP", "rtmp"), 0);
	assert_int_equal(count("Rtmp;rtmp;RTMP", "rtmp"), 1);
	/* nothing is trimmed */
	assert_int_equal(count(" RTMP", "RTMP"), 0);
	assert_int_equal(count("RTMP ", "RTMP"), 0);
	assert_int_equal(count("WHIP; RIST", " RIST"), 1);
	assert_int_equal(count("WHIP; RIST", "RIST"), 0);
	assert_int_equal(count("\tRTMP", "RTMP"), 0);
}

/* Characterized, not endorsed: a protocol listed twice counts twice, so
 * enum_cb gets the same output id twice. */
static void duplicates(void **state)
{
	(void)state;
	assert_int_equal(count("RTMP;RTMP", "RTMP"), 2);
	assert_int_equal(count("RTMP;RTMPS;RTMP", "RTMP"), 2);
	assert_int_equal(count("RTMP;RTMP;RTMP", "RTMP"), 3);
	assert_int_equal(count("RTMP;rtmp;RTMP", "RTMP"), 2);
}

static void empty_and_trailing_segments(void **state)
{
	(void)state;
	assert_int_equal(count(";RTMP;;", "RTMP"), 1);
	assert_int_equal(count("HLS;", "HLS"), 1);
	assert_int_equal(count(";HLS", "HLS"), 1);
	assert_int_equal(count(";;;", "RTMP"), 0);
	assert_int_equal(count(";", "RTMP"), 0);
}

/* Characterized, not endorsed: the empty protocol matches each empty
 * segment that a ';' ends, but not a trailing one (the scan stops at the
 * NUL). libobs never asks for it: "" is never a registered protocol, and the
 * caller returns before the match. */
static void empty_protocol(void **state)
{
	(void)state;
	assert_int_equal(count(";", ""), 1);
	assert_int_equal(count(";;", ""), 2);
	assert_int_equal(count(";RTMP;;", ""), 2);
	assert_int_equal(count(";A", ""), 1);
	assert_int_equal(count("A;", ""), 0);
	assert_int_equal(count("A;;B", ""), 1);
	assert_int_equal(count("A", ""), 0);
}

/* Bytes, not text: high bytes, bytes that are not UTF-8, and control
 * bytes compare as they are. */
static void bytes(void **state)
{
	(void)state;
	assert_int_equal(count("\xff;\xfe", "\xff"), 1);
	assert_int_equal(count("\xff;\xfe", "\xfe"), 1);
	assert_int_equal(count("\xff\xfe", "\xff"), 0);
	assert_int_equal(count("\xc3\xa9;e\xcc\x81", "\xc3\xa9"), 1);
	assert_int_equal(count("\xc3\xa9;e\xcc\x81", "e\xcc\x81"), 1);
	assert_int_equal(count("\x01\x7f;\x80", "\x80"), 1);
	assert_int_equal(count("%s;%n", "%n"), 1);
}

static void long_lists(void **state)
{
	(void)state;

	/* 100000 copies of "X" */
	const size_t copies = 100000;
	char *list = malloc(copies * 2);
	assert_non_null(list);
	for (size_t i = 0; i < copies; i++) {
		list[2 * i] = 'X';
		list[2 * i + 1] = ';';
	}
	list[copies * 2 - 1] = '\0';
	assert_int_equal(count(list, "X"), copies);
	assert_int_equal(count(list, "XX"), 0);
	free(list);

	/* one 1 MiB segment, and the same protocol one byte shorter */
	const size_t len = 1024 * 1024;
	char *big = malloc(len + 3);
	char *protocol = malloc(len + 1);
	assert_non_null(big);
	assert_non_null(protocol);
	memset(big, 'P', len);
	memcpy(big + len, ";Q", 3);
	memset(protocol, 'P', len);
	protocol[len] = '\0';
	assert_int_equal(count(big, protocol), 1);
	protocol[len - 1] = '\0';
	assert_int_equal(count(big, protocol), 0);
	free(big);
	free(protocol);
}

/* A model of the match: split at every ';' up to the NUL, without the
 * segment after a trailing ';', and count the segments equal to protocol. */
static size_t model_count(const char *list, const char *protocol)
{
	size_t n = strlen(list);
	size_t plen = strlen(protocol);
	size_t matches = 0;
	size_t start = 0;
	while (start < n) {
		size_t end = start;
		while (end < n && list[end] != ';')
			end++;
		if (end - start == plen && memcmp(list + start, protocol, plen) == 0)
			matches++;
		start = end + 1;
	}
	return matches;
}

/* Every list of up to 7 bytes over {'A', 'B', ';'} against every protocol
 * of up to 3 bytes over {'A', 'B', ';'}, the empty one included. */
static void exhaustive_small(void **state)
{
	(void)state;
	static const char alphabet[] = {'A', 'B', ';'};
	char list[8];
	char protocol[4];
	size_t checked = 0;

	for (size_t llen = 0; llen <= 7; llen++) {
		size_t lcombos = 1;
		for (size_t i = 0; i < llen; i++)
			lcombos *= 3;
		for (size_t l = 0; l < lcombos; l++) {
			size_t v = l;
			for (size_t i = 0; i < llen; i++, v /= 3)
				list[i] = alphabet[v % 3];
			list[llen] = '\0';

			for (size_t plen = 0; plen <= 3; plen++) {
				size_t pcombos = 1;
				for (size_t i = 0; i < plen; i++)
					pcombos *= 3;
				for (size_t p = 0; p < pcombos; p++) {
					size_t w = p;
					for (size_t i = 0; i < plen; i++, w /= 3)
						protocol[i] = alphabet[w % 3];
					protocol[plen] = '\0';

					size_t got = count(list, protocol);
					size_t want = model_count(list, protocol);
					if (got != want)
						fprintf(stderr, "list \"%s\" protocol \"%s\": %zu, model %zu\n", list,
							protocol, got, want);
					assert_int_equal(got, want);
					checked++;
				}
			}
		}
	}
	/* (3^8 - 1) / 2 lists times (3^4 - 1) / 2 protocols */
	assert_int_equal(checked, 3280 * 40);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_and_empty_list),
		cmocka_unit_test(whole_segments),
		cmocka_unit_test(case_and_spaces),
		cmocka_unit_test(duplicates),
		cmocka_unit_test(empty_and_trailing_segments),
		cmocka_unit_test(empty_protocol),
		cmocka_unit_test(bytes),
		cmocka_unit_test(long_lists),
		cmocka_unit_test(exhaustive_small),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
