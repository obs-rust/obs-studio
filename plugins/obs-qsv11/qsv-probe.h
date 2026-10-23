#pragma once

/* What obs-qsv11's Linux adapter probe decides from what VA-API reported,
 * kept free of VA-API, VPL and libobs graphics so it can be replayed in a
 * unit test (obs-rust/obs-studio#1863). We have no Intel GPU in CI: the probe
 * logs each device as a "QSV probe:" line, a log from real hardware is
 * checked in as a fixture, and the test replays those lines through the
 * same decision the plugin makes. */

#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* One render device as the probe saw it. */
struct qsv_probe_facts {
	/* The device path, "/dev/dri/renderD128". */
	char node[64];
	/* vaInitialize() succeeded on it. */
	bool opened;
	/* vaQueryVendorString(), empty when not opened. */
	char driver[160];
	/* HEVC Main / AV1 Profile 0 encode with CBR, CQP or VBR, on the
	 * EncSlice or EncSliceLP entrypoint. */
	bool hevc;
	bool av1;
};

struct qsv_probe_adapter {
	bool is_intel;
	bool supports_hevc;
	bool supports_av1;
};

/* The encoder families obs_module_load() registers. */
struct qsv_probe_codecs {
	bool avc;
	bool hevc;
	bool av1;
};

/* Whether the device is an adapter QSV can use, and what it encodes. False
 * when it did not open, or runs the legacy i965 driver (QSV needs iHD). */
bool qsv_probe_adapter_from_facts(const struct qsv_probe_facts *facts, struct qsv_probe_adapter *out);

void qsv_probe_codecs_from_adapters(const struct qsv_probe_adapter *adapters, size_t count,
				    struct qsv_probe_codecs *out);

/* The log line for facts, after the "QSV probe: " prefix:
 * node=/dev/dri/renderD128 opened=1 hevc=1 av1=0 driver=Intel iHD driver ...
 * The driver comes last, so it may contain spaces. */
void qsv_probe_format(const struct qsv_probe_facts *facts, char *buf, size_t size);

/* Parses a line qsv_probe_format() wrote, with or without anything before
 * the "QSV probe: " prefix (a log file line). False when it is not one. */
bool qsv_probe_parse(const char *line, struct qsv_probe_facts *out);

#define QSV_PROBE_LOG_PREFIX "QSV probe: "

#ifdef __cplusplus
}
#endif
