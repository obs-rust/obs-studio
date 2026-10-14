/* Settings > Output: the Replay Buffer memory estimate label
 * (obsproject/obs-studio#13171). The helper the settings dialog calls is run
 * on Qt's offscreen platform (no display) with a style sheet that colours the
 * "text-warning" class like the themes do. Raising the maximum replay time
 * above the recommended memory shows the estimate as a warning; lowering it
 * again must show the plain estimate in the normal colour. */

#include <QApplication>
#include <QColor>
#include <QLabel>
#include <QPalette>
#include <QString>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
extern "C" {
#include <cmocka.h>
}

#include "replay-buffer-estimate.hpp"

static const QColor normal_colour(255, 255, 255);
static const QColor warning_colour(255, 200, 0);

static QLabel *label;

static int setup(void **)
{
	label = new QLabel();
	label->setObjectName("advRBEstimate");
	label->ensurePolished();
	return 0;
}

static int teardown(void **)
{
	delete label;
	label = nullptr;
	return 0;
}

static QString label_class()
{
	return label->property("class").toString();
}

static QColor text_colour()
{
	return label->palette().color(label->foregroundRole());
}

static void warning_shown_above_the_limit(void **)
{
	SetReplayBufferEstimate(label, "Warning: estimate above the maximum", true);

	assert_string_equal(qPrintable(label_class()), "text-warning");
	assert_true(text_colour() == warning_colour);
}

static void warning_cleared_when_back_within_the_limit(void **)
{
	SetReplayBufferEstimate(label, "Warning: estimate above the maximum", true);
	SetReplayBufferEstimate(label, "Estimated memory usage: 119 MB", false);

	assert_string_equal(qPrintable(label->text()), "Estimated memory usage: 119 MB");
	assert_string_equal(qPrintable(label_class()), "");
	assert_true(text_colour() == normal_colour);
}

static void warning_cleared_when_the_estimate_is_unknown(void **)
{
	SetReplayBufferEstimate(label, "Warning: estimate above the maximum", true);
	SetReplayBufferEstimate(label, "Cannot estimate memory usage", false);

	assert_string_equal(qPrintable(label_class()), "");
	assert_true(text_colour() == normal_colour);
}

static void normal_estimate_keeps_the_normal_colour(void **)
{
	SetReplayBufferEstimate(label, "Estimated memory usage: 358 MB", false);

	assert_string_equal(qPrintable(label_class()), "");
	assert_true(text_colour() == normal_colour);
}

int main(int argc, char **argv)
{
	qputenv("QT_QPA_PLATFORM", "offscreen");
	QApplication app(argc, argv);
	app.setStyleSheet("QLabel { color: rgb(255, 255, 255); }"
			  "QLabel[class~=\"text-warning\"] { color: rgb(255, 200, 0); }");

	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(warning_shown_above_the_limit, setup, teardown),
		cmocka_unit_test_setup_teardown(warning_cleared_when_back_within_the_limit, setup, teardown),
		cmocka_unit_test_setup_teardown(warning_cleared_when_the_estimate_is_unknown, setup, teardown),
		cmocka_unit_test_setup_teardown(normal_estimate_keeps_the_normal_colour, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
