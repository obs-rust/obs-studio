/* SPDX-License-Identifier: GPL-2.0-or-later */
/* Plugin-side pure C decision, like other host-independent plugin regressions
 * in test/cmocka. No Win32 enumeration or libobs ABI is exercised here. */
#include "plugins/win-capture/monitor-identity.h"

static unsigned failures;
static unsigned checks;

#define CHECK(condition, label) \
	do { \
		++checks; \
		if (!(condition)) { \
			fprintf(stderr, "FAIL: %s\n", label); \
			++failures; \
		} \
	} while (0)

static void edid_checksum(uint8_t edid[128])
{
	unsigned sum = 0;
	for (size_t i = 0; i < 127; ++i)
		sum += edid[i];
	edid[127] = (uint8_t)(0u - sum);
}

static void edid_cases(void)
{
	uint8_t edid[128] = {0, 255, 255, 255, 255, 255, 255, 0, 0x10, 0xac, 0x34, 0x12, 1, 2, 3, 4};
	char identity[64];
	edid_checksum(edid);
	CHECK(monitor_edid_identity(edid, sizeof(edid), identity), "valid EDID supplies identity");
	CHECK(strcmp(identity, "10AC:1234:N04030201") == 0, "EDID byte order and full numeric serial");
	CHECK(!monitor_edid_identity(edid, 127, identity) && !*identity, "truncated EDID has no identity");
	edid[20] ^= 1;
	CHECK(!monitor_edid_identity(edid, sizeof(edid), identity) && !*identity, "bad EDID checksum has no identity");
	edid[20] ^= 1;
	memset(edid + 12, 0, 4);
	edid_checksum(edid);
	CHECK(!monitor_edid_identity(edid, sizeof(edid), identity) && !*identity,
	      "model alone without serial cannot recover a monitor");
	edid[57] = 255;
	memcpy(edid + 59, "SERIAL-A     ", 13);
	edid_checksum(edid);
	CHECK(monitor_edid_identity(edid, sizeof(edid), identity) && strcmp(identity, "10AC:1234:TSERIAL-A") == 0,
	      "EDID descriptor serial is supported and trailing padding removed");
	edid[59] = 1;
	edid_checksum(edid);
	CHECK(!monitor_edid_identity(edid, sizeof(edid), identity) && !*identity,
	      "nonprintable descriptor serial is rejected");
	edid[0] = 1;
	edid_checksum(edid);
	CHECK(!monitor_edid_identity(edid, sizeof(edid), identity) && !*identity, "invalid EDID header is rejected");
}

int main(void)
{
	struct duplicator_monitor_info monitors[2] = {0};
	monitor_fill_info(&monitors[0], "interface-new-port", "DISPLAY1", "Monitor A", "DEL:1234:SERIAL-A");
	monitor_fill_info(&monitors[1], "interface-other", "DISPLAY2", "Monitor B", "DEL:1234:SERIAL-B");

	CHECK(monitor_matches(&monitors[0], "interface-new-port", ""), "same interface id matches");
	CHECK(monitor_find(monitors, 2, "interface-new-port", "DEL:1234:SERIAL-B") == 0, "interface id wins over EDID");
	CHECK(monitor_find(monitors, 2, "DISPLAY2", "DEL:1234:SERIAL-A") == 1, "GDI name is first fallback");
	CHECK(monitor_find(monitors, 2, "interface-old-port", "DEL:1234:SERIAL-A") == 0,
	      "changed interface id with unique EDID identity matches");
	CHECK(monitor_find(monitors, 2, "unknown", "DEL:1234:UNKNOWN") == 2, "unknown monitor matches nothing");
	CHECK(monitor_find(monitors, 2, "unknown", "") == 2, "missing EDID matches nothing");

	monitor_fill_info(&monitors[1], "interface-twin", "DISPLAY2", "Monitor A twin", "DEL:1234:SERIAL-A");
	CHECK(monitor_find(monitors, 2, "interface-old-port", "DEL:1234:SERIAL-A") == 2,
	      "duplicate EDID identity matches nothing");
	CHECK(monitor_find(monitors, 2, "interface-new-port", "DEL:1234:SERIAL-A") == 0,
	      "primary key still matches with duplicate EDID");
	edid_cases();
	printf("Windows monitor identity: %u checks, %u failures\n", checks, failures);
	return failures ? 1 : 0;
}

#undef CHECK
