// The C++ and Qt headers go first: cmocka.h defines macros (fail, ...) that
// clash with libstdc++.
#include <QByteArray>
#include <QString>

#include "forms/SessionDuration.h"

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
// The Windows cmocka.h has no extern "C" guard of its own.
extern "C" {
#include <cmocka.h>
}

/* Tools > WebSocket Server Settings lists the connected clients with their
 * Session Duration as hh:mm:ss. The dialog formatted it with
 * QTime(0, 0, seconds), which is only valid for 0 to 59 seconds: from one
 * minute on the QTime was invalid and the cell was empty for the rest of the
 * session (upstream obsproject/obs-websocket#1208). The dialog's
 * FillSessionTable() calls FormatSessionDuration() from the plugin's
 * forms/SessionDuration.h; this builds that header on its own. */

static void check(qint64 seconds, const char *expected)
{
	QByteArray text = FormatSessionDuration(seconds).toUtf8();
	assert_string_equal(text.constData(), expected);
}

/* Control: the first minute was right already. */
static void under_a_minute(void **state)
{
	(void)state;
	check(0, "00:00:00");
	check(30, "00:00:30");
	check(59, "00:00:59");
}

/* The upstream report: the cell went empty at one minute. */
static void one_minute_and_more(void **state)
{
	(void)state;
	check(60, "00:01:00");
	check(70, "00:01:10");
	check(3599, "00:59:59");
}

static void hours(void **state)
{
	(void)state;
	check(3600, "01:00:00");
	check(3 * 3600 + 25 * 60 + 7, "03:25:07");
	check(86399, "23:59:59");
}

/* A session longer than a day: the hours count on, they do not wrap to 00
 * (QTime(0, 0).addSecs() would). */
static void more_than_a_day(void **state)
{
	(void)state;
	check(86400, "24:00:00");
	check(100 * 3600 + 61, "100:01:01");
}

/* The system clock set back below the connect time: 00:00:00, not an
 * empty cell or a negative time. */
static void clock_set_back(void **state)
{
	(void)state;
	check(-5, "00:00:00");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(under_a_minute),  cmocka_unit_test(one_minute_and_more), cmocka_unit_test(hours),
		cmocka_unit_test(more_than_a_day), cmocka_unit_test(clock_set_back),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
