#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include "qsv-probe.h"
#include "fixtures/qsv-probe-hd530.h"

/* obs-rust/obs-studio#1863: obs-qsv11 decides which Intel QSV encoders to
 * register from what VA-API reports for each render device. No CI runner has
 * an Intel GPU, so the plugin logs each device as a "QSV probe:" line and the
 * test replays lines from a log on real hardware through the same decision.
 * A change to what the probe reads or decides fails here, and the hardware
 * run on #1863 has to be repeated before the fixture changes. */

static size_t replay(const char *const *log, struct qsv_probe_adapter *adapters, size_t max)
{
	size_t count = 0;
	for (; *log; log++) {
		struct qsv_probe_facts facts;
		if (!qsv_probe_parse(*log, &facts)) {
			continue;
		}
		if (count < max && qsv_probe_adapter_from_facts(&facts, &adapters[count])) {
			count++;
		}
	}
	return count;
}

static void test_hd530_registers_avc_and_hevc(void **state)
{
	(void)state;
	struct qsv_probe_adapter adapters[4];
	struct qsv_probe_codecs codecs;

	size_t count = replay(qsv_probe_hd530_log, adapters, 4);
	assert_int_equal(count, 1);
	assert_true(adapters[0].is_intel);

	qsv_probe_codecs_from_adapters(adapters, count, &codecs);
	assert_true(codecs.avc);
	assert_true(codecs.hevc);
	assert_false(codecs.av1);
}

/* A probe line survives a format and parse round trip, driver spaces too. */
static void test_format_parse_round_trip(void **state)
{
	(void)state;
	struct qsv_probe_facts in = {"/dev/dri/renderD129", true,
				     "Intel iHD driver for Intel(R) Gen Graphics - 25.2.3 ()", false, true};
	struct qsv_probe_facts out;
	char line[300];

	qsv_probe_format(&in, line, sizeof(line));
	assert_string_equal(line, "node=/dev/dri/renderD129 opened=1 hevc=0 av1=1 driver=Intel iHD driver for "
				  "Intel(R) Gen Graphics - 25.2.3 ()");

	char logged[320];
	snprintf(logged, sizeof(logged), "info: " QSV_PROBE_LOG_PREFIX "%s", line);
	assert_true(qsv_probe_parse(logged, &out));
	assert_string_equal(out.node, in.node);
	assert_true(out.opened);
	assert_string_equal(out.driver, in.driver);
	assert_false(out.hevc);
	assert_true(out.av1);

	assert_false(qsv_probe_parse("info: VAAPI: something else", &out));
	assert_false(qsv_probe_parse("info: QSV probe: opened=1", &out));
}

/* The legacy i965 driver cannot run QSV, a device that did not open is not an
 * adapter, and a non-Intel driver is an adapter QSV does not encode on. */
static void test_rejected_and_foreign_devices(void **state)
{
	(void)state;
	static const char *const log[] = {
		"info: QSV probe: node=/dev/dri/renderD128 opened=1 hevc=0 av1=0 driver=Intel i965 driver for Intel(R) Skylake - 2.4.1",
		"info: QSV probe: node=/dev/dri/renderD129 opened=0 hevc=0 av1=0 driver=",
		"info: QSV probe: node=/dev/dri/renderD130 opened=1 hevc=1 av1=1 driver=Mesa Gallium driver 24.0.9 for AMD Radeon RX 7600",
		NULL,
	};
	struct qsv_probe_adapter adapters[4];
	struct qsv_probe_codecs codecs;

	size_t count = replay(log, adapters, 4);
	assert_int_equal(count, 1);
	assert_false(adapters[0].is_intel);

	qsv_probe_codecs_from_adapters(adapters, count, &codecs);
	assert_false(codecs.avc);
	assert_false(codecs.hevc);
	assert_false(codecs.av1);
}

/* An Intel iGPU plus an Arc dGPU: the codecs are the union, AV1 from Arc. */
static void test_codecs_are_the_union(void **state)
{
	(void)state;
	static const char *const log[] = {
		"info: QSV probe: node=/dev/dri/renderD128 opened=1 hevc=1 av1=0 driver=Intel iHD driver for Intel(R) Gen Graphics - 24.1.0 ()",
		"info: QSV probe: node=/dev/dri/renderD129 opened=1 hevc=1 av1=1 driver=Intel iHD driver for Intel(R) Gen Graphics - 24.1.0 ()",
		NULL,
	};
	struct qsv_probe_adapter adapters[4];
	struct qsv_probe_codecs codecs;

	size_t count = replay(log, adapters, 4);
	assert_int_equal(count, 2);
	qsv_probe_codecs_from_adapters(adapters, count, &codecs);
	assert_true(codecs.avc);
	assert_true(codecs.hevc);
	assert_true(codecs.av1);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_hd530_registers_avc_and_hevc),
		cmocka_unit_test(test_format_parse_round_trip),
		cmocka_unit_test(test_rejected_and_foreign_devices),
		cmocka_unit_test(test_codecs_are_the_union),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
