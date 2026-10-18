/* Exit in the tray menu did nothing when OBS was started with
 * --minimize-to-tray on Wayland and its window had never been shown
 * (obsproject/obs-studio#13494, reported by zorbathut). The main window
 * then has a QWindow but no platform window yet. For a top-level widget,
 * QWidget::close() asks QWindow::close(), which returns true without sending
 * a close event when there is no platform window, so OBSBasic::closeEvent()
 * never ran and OBS kept running.
 *
 * OBSBasic::close() goes through CloseTopLevel()
 * (frontend/utility/close-top-level.hpp). The test puts a top-level widget
 * into the same state (created, then its platform window destroyed) and
 * checks that it still gets its close event. Qt's offscreen platform (minimal
 * on Windows), no display. */

#include <QApplication>
#include <QCloseEvent>
#include <QDir>
#include <QLibraryInfo>
#include <QWidget>
#include <QWindow>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
extern "C" {
#include <cmocka.h>
}

#include "close-top-level.hpp"

class Window : public QWidget {
public:
	int closeEvents = 0;
	bool accept = true;

protected:
	void closeEvent(QCloseEvent *event) override
	{
		closeEvents++;
		if (accept) {
			event->accept();
		} else {
			event->ignore();
		}
	}
};

/* cmocka's fail() longjmps past C++ destructors, so each case's window lives
 * on the heap and teardown deletes it. */
static Window *w;

static int setup(void **state)
{
	(void)state;
	w = new Window;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	delete w;
	w = nullptr;
	return 0;
}

/* Created but never shown, with no platform window: the --minimize-to-tray
 * main window on Wayland. */
static void make_handle_without_platform_window(void)
{
	(void)w->winId();
	QWindow *window = w->windowHandle();
	if (!window) {
		fprintf(stderr, "winId() made no QWindow\n");
		fail();
	}
	window->destroy();
	if (window->handle()) {
		fprintf(stderr, "the QWindow still has a platform window\n");
		fail();
	}
}

static void never_shown_window_gets_close_event(void **state)
{
	(void)state;
	make_handle_without_platform_window();

	bool closed = CloseTopLevel(w);
	if (w->closeEvents != 1) {
		fprintf(stderr, "close events: %d (expected 1); CloseTopLevel returned %d\n", w->closeEvents, closed);
		fail();
	}
	assert_true(closed);
}

static void never_shown_window_can_refuse(void **state)
{
	(void)state;
	w->accept = false;
	make_handle_without_platform_window();

	bool closed = CloseTopLevel(w);
	if (w->closeEvents != 1) {
		fprintf(stderr, "close events: %d (expected 1)\n", w->closeEvents);
		fail();
	}
	assert_false(closed);
}

static void shown_window_closes(void **state)
{
	(void)state;
	w->show();
	QApplication::processEvents();

	assert_true(CloseTopLevel(w));
	assert_int_equal(w->closeEvents, 1);
	assert_false(w->isVisible());
}

static void shown_window_can_refuse(void **state)
{
	(void)state;
	w->accept = false;
	w->show();
	QApplication::processEvents();

	assert_false(CloseTopLevel(w));
	assert_int_equal(w->closeEvents, 1);
	assert_true(w->isVisible());
}

static void window_without_qwindow_closes(void **state)
{
	(void)state;
	assert_null(w->windowHandle());

	assert_true(CloseTopLevel(w));
	assert_int_equal(w->closeEvents, 1);
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
	/* The Windows Qt from obs-deps ships no qoffscreen.dll (no FreeType). */
	qputenv("QT_QPA_PLATFORM", "minimal");
	if (!platform_plugin_found("minimal")) {
		return 1;
	}
#else
	qputenv("QT_QPA_PLATFORM", "offscreen");
#endif
	QApplication app(argc, argv);

	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(never_shown_window_gets_close_event, setup, teardown),
		cmocka_unit_test_setup_teardown(never_shown_window_can_refuse, setup, teardown),
		cmocka_unit_test_setup_teardown(shown_window_closes, setup, teardown),
		cmocka_unit_test_setup_teardown(shown_window_can_refuse, setup, teardown),
		cmocka_unit_test_setup_teardown(window_without_qwindow_closes, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
