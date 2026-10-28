/* SPDX-License-Identifier: GPL-2.0-or-later */
/* Plugin-side pure C decision, like other host-independent plugin regressions
 * in test/cmocka. No Win32 enumeration or libobs ABI is exercised here. */
#include "plugins/win-capture/monitor-identity.h"

static unsigned failures;

#define CHECK(condition, label)                         \
	do {                                             \
		if (!(condition)) {                       \
			fprintf(stderr, "FAIL: %s\n", label); \
			++failures;                       \
		}                                        \
	} while (0)

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
	printf("Windows monitor identity: 8 checks, %u failures\n", failures);
	return failures ? 1 : 0;
}

#undef CHECK
