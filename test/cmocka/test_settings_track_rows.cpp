/* Settings > Output: in a tall Settings window the audio track buttons slid
 * away from their "Audio Track" label (obsproject/obs-studio#13286).
 *
 * Each track row is a QStackedWidget (single-track radio buttons or
 * multi-track checkboxes) next to a label in a form layout. With a vertical
 * size policy of Preferred, the page's spare height went to the stack, the
 * buttons were centred in it, and the label stayed at the top of the row.
 * In the 0.1.0-beta.20261014 release, in a 2000 px tall window, they were
 * 190 px apart on Advanced > Streaming, 269 px on Advanced > Recording and
 * 163 px for the Simple mode recording tracks.
 *
 * This builds the real frontend/forms/OBSBasicSettings.ui (uic) with
 * stand-ins for its custom widgets (settings-ui-stubs/), shows the Output
 * page at the default height and at a tall one, and checks that the first
 * track button stays level with its label. Qt's offscreen platform, no
 * display, no frontend build and no libobs. */

#include <QApplication>
#include <QDialog>
#include <QDir>
#include <QLabel>
#include <QLibraryInfo>
#include <QListWidget>
#include <QStackedWidget>
#include <QTabWidget>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
extern "C" {
#include <cmocka.h>
}

#include "ui_OBSBasicSettings.h"

/* Output page row in the settings list (OBSBasicSettings::Pages::OUTPUT). */
static const int output_page_row = 3;

struct Row {
	int mode_page; /* outputModePages: 0 Simple, 1 Advanced */
	int adv_tab;   /* advOutTabs: 0 Streaming, 1 Recording */
	QLabel *Ui::OBSBasicSettings::*label;
	QStackedWidget *Ui::OBSBasicSettings::*stack;
	QWidget *(*first)(Ui::OBSBasicSettings &ui);
};

struct Fixture {
	QDialog dialog;
	Ui::OBSBasicSettings ui;
};

static int setup(void **state)
{
	auto *f = new Fixture;
	f->ui.setupUi(&f->dialog);
	*state = f;
	return 0;
}

static int teardown(void **state)
{
	delete static_cast<Fixture *>(*state);
	return 0;
}

static void settle()
{
	for (int i = 0; i < 5; i++) {
		QApplication::processEvents();
	}
}

static int center_y(QDialog &dialog, QWidget *w)
{
	return w->mapTo(&dialog, QPoint(0, w->height() / 2)).y();
}

/* Shows the row's page at each height and returns the largest distance
 * between the first track button's centre and its label's centre. */
static int worst_drift(Fixture *f, const Row &row)
{
	Ui::OBSBasicSettings &ui = f->ui;
	ui.listWidget->setCurrentRow(output_page_row);
	ui.settingsPages->setCurrentWidget(ui.outputPage);
	ui.outputModePages->setCurrentIndex(row.mode_page);
	ui.advOutTabs->setCurrentIndex(row.adv_tab);
	(ui.*row.stack)->setCurrentIndex(0);

	QLabel *label = ui.*row.label;
	QWidget *button = row.first(ui);
	f->dialog.show();

	int worst = 0;
	for (int height : {700, 1400, 2000}) {
		f->dialog.resize(f->dialog.width(), height);
		settle();
		assert_true(button->isVisible());
		assert_true(label->isVisible());
		int drift = center_y(f->dialog, button) - center_y(f->dialog, label);
		printf("  window height %d: button centre %d px from the label's (stack %d px tall, wants %d)\n",
		       f->dialog.height(), drift, (ui.*row.stack)->height(), (ui.*row.stack)->sizeHint().height());
		worst = qMax(worst, qAbs(drift));
	}
	return worst;
}

/* A pixel or two of rounding between a label and a radio button of the same
 * row is fine; the bug moved them hundreds of pixels apart. */
static const int max_drift = 3;

/* The tests below report with fprintf and a plain fail(), not fail_msg():
 * the Windows cmocka library does not export the printer that fail_msg
 * expands to (#1580). */

static void advanced_streaming_track_stays_by_label(void **state)
{
	Row row{1, 0, &Ui::OBSBasicSettings::advStreamTrackWidgetLabel, &Ui::OBSBasicSettings::advStreamTrackWidget,
		[](Ui::OBSBasicSettings &ui) -> QWidget * { return ui.advOutTrack1; }};
	const int drift = worst_drift(static_cast<Fixture *>(*state), row);
	if (drift < 0 || drift > max_drift) {
		fprintf(stderr, "first button is %d px from its label (allowed: %d)\n", drift, max_drift);
		fail();
	}
}

static void advanced_recording_tracks_stay_by_label(void **state)
{
	Row row{1, 1, &Ui::OBSBasicSettings::label_29, &Ui::OBSBasicSettings::advRecTrackWidget,
		[](Ui::OBSBasicSettings &ui) -> QWidget * { return ui.advOutRecTrack1; }};
	const int drift = worst_drift(static_cast<Fixture *>(*state), row);
	if (drift < 0 || drift > max_drift) {
		fprintf(stderr, "first button is %d px from its label (allowed: %d)\n", drift, max_drift);
		fail();
	}
}

static void simple_recording_tracks_stay_by_label(void **state)
{
	Row row{0, 0, &Ui::OBSBasicSettings::simplerectrack_label, &Ui::OBSBasicSettings::simpleRecTrackWidget,
		[](Ui::OBSBasicSettings &ui) -> QWidget * { return ui.simpleOutRecTrack1; }};
	const int drift = worst_drift(static_cast<Fixture *>(*state), row);
	if (drift < 0 || drift > max_drift) {
		fprintf(stderr, "first button is %d px from its label (allowed: %d)\n", drift, max_drift);
		fail();
	}
}

#ifdef _WIN32
/* With no platform plugin, a release Qt on Windows shows a modal message box
 * before it exits when the process has no console window, and under ctest on
 * a CI runner nobody closes it. Look for the plugin where Qt does
 * (QT_QPA_PLATFORM_PLUGIN_PATH, then <library path>/platforms) and fail on
 * stderr instead. */
static bool platform_plugin_found(const char *name)
{
	const QString file =
		QStringLiteral("q%1%2.dll")
			.arg(QString::fromLatin1(name), QLibraryInfo::isDebugBuild() ? QStringLiteral("d") : QString());
	QStringList dirs;
	const QString plugin_path = qEnvironmentVariable("QT_QPA_PLATFORM_PLUGIN_PATH");
	if (!plugin_path.isEmpty()) {
		dirs.append(plugin_path);
	}
	for (const QString &dir : QCoreApplication::libraryPaths()) {
		dirs.append(dir + QStringLiteral("/platforms"));
	}
	for (const QString &dir : dirs) {
		if (QDir(dir).exists(file)) {
			return true;
		}
	}
	fprintf(stderr, "Qt platform plugin %s not found; looked in:\n", qPrintable(file));
	for (const QString &dir : dirs) {
		fprintf(stderr, "  %s\n", qPrintable(QDir::toNativeSeparators(dir)));
	}
	return false;
}
#endif

int main(int argc, char **argv)
{
#ifdef _WIN32
	/* Qt builds its offscreen platform only with FreeType, and the Windows
	 * Qt from obs-deps has none, so it ships no qoffscreen.dll. The minimal
	 * platform lays widgets out the same way; enable_fonts gives it the
	 * system fonts. */
	qputenv("QT_QPA_PLATFORM", "minimal:enable_fonts");
	if (!platform_plugin_found("minimal")) {
		return 1;
	}
#else
	qputenv("QT_QPA_PLATFORM", "offscreen");
#endif
	QApplication app(argc, argv);

	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(advanced_streaming_track_stays_by_label, setup, teardown),
		cmocka_unit_test_setup_teardown(advanced_recording_tracks_stay_by_label, setup, teardown),
		cmocka_unit_test_setup_teardown(simple_recording_tracks_stay_by_label, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
