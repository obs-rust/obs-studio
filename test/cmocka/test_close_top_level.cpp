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
 * on Windows), no display.
 *
 * Cmd+Q on macOS did nothing when a main dock floated (obs-rust/obs-studio#5406):
 * an application quit closes the top-level windows in no fixed order, the
 * floating main dock (not closable) refused while the main window was
 * visible, and Qt cancelled the quit. The quit cases below check that
 * QuitClosesMainWindowFirst closes the main window first.
 *
 * On macOS a floating dock disappeared whenever another app was in front
 * (obs-rust/obs-studio#5409): Qt hides a tool window while the application is
 * inactive unless it has Qt::WA_MacAlwaysShowToolWindow.
 * floating_dock_stays_shown_while_inactive checks that KeepFloatingDockShown()
 * (frontend/utility/dock-window.hpp) sets it on a dock that then floats. */

#include <QApplication>
#include <QCloseEvent>
#include <QDir>
#include <QDockWidget>
#include <QLibraryInfo>
#include <QMainWindow>
#include <QWidget>
#include <QWindow>

#include <vector>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
extern "C" {
#include <cmocka.h>
}

#include "close-top-level.hpp"
#include "dock-window.hpp"

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

/* The main window and floating main docks of OBS: QMainWindow with docks
 * that are movable and floatable but not closable (OBSBasic's mainFeatures). */
class Main : public QMainWindow {
public:
	int closeEvents = 0;
	bool accept = true;

protected:
	void closeEvent(QCloseEvent *event) override
	{
		closeEvents++;
		if (accept) {
			QMainWindow::closeEvent(event);
		} else {
			event->ignore();
		}
	}
};

class MainDock : public QDockWidget {
public:
	MainDock(const QString &title, Main *main) : QDockWidget(title, main), main(main) {}
	int closeEvents = 0;
	bool mainWasVisible = false;

protected:
	void closeEvent(QCloseEvent *event) override
	{
		closeEvents++;
		mainWasVisible = mainWasVisible || main->isVisible();
		QDockWidget::closeEvent(event);
	}

private:
	Main *main;
};

/* Enough floating docks that, in Qt's close order, one of them comes before
 * the main window in nearly every run. */
static constexpr int floating_docks = 32;

static Main *main_window;
static std::vector<MainDock *> docks;
static QuitClosesMainWindowFirst *quit_filter;

static int quit_setup(void **state)
{
	(void)state;
	main_window = new Main;
	main_window->setCentralWidget(new QWidget);
	main_window->resize(800, 600);
	for (int i = 0; i < floating_docks; i++) {
		auto *dock = new MainDock(QStringLiteral("dock %1").arg(i), main_window);
		dock->setObjectName(QStringLiteral("dock%1").arg(i));
		dock->setFeatures(QDockWidget::DockWidgetMovable | QDockWidget::DockWidgetFloatable);
		dock->setWidget(new QWidget);
		main_window->addDockWidget(Qt::LeftDockWidgetArea, dock);
		docks.push_back(dock);
	}
	main_window->show();
	QApplication::processEvents();
	for (MainDock *dock : docks) {
		dock->setFloating(true);
		dock->show();
	}
	QApplication::processEvents();
	quit_filter = new QuitClosesMainWindowFirst(main_window, qApp);
	qApp->installEventFilter(quit_filter);
	return 0;
}

static int quit_teardown(void **state)
{
	(void)state;
	qApp->removeEventFilter(quit_filter);
	delete quit_filter;
	quit_filter = nullptr;
	delete main_window;
	main_window = nullptr;
	docks.clear();
	QApplication::processEvents();
	return 0;
}

/* What Cmd+Q, logoff and QCoreApplication::quit() send. Returns whether the
 * quit went through. */
static bool send_quit(void)
{
	QEvent quit(QEvent::Quit);
	QCoreApplication::sendEvent(qApp, &quit);
	QApplication::processEvents();
	return quit.isAccepted();
}

static int visible_docks(void)
{
	int n = 0;
	for (MainDock *dock : docks) {
		n += dock->isVisible() ? 1 : 0;
	}
	return n;
}

static void quit_closes_the_main_window_with_floating_main_docks(void **state)
{
	(void)state;
	const bool accepted = send_quit();
	int refused = 0;
	for (MainDock *dock : docks) {
		refused += dock->mainWasVisible ? 1 : 0;
	}
	if (!accepted || main_window->closeEvents != 1 || main_window->isVisible() || visible_docks() != 0) {
		fprintf(stderr,
			"quit accepted %d, main window close events %d (expected 1), main window visible %d, "
			"docks still visible %d of %d, docks closed while the main window was visible %d\n",
			accepted, main_window->closeEvents, main_window->isVisible(), visible_docks(), floating_docks,
			refused);
		fail();
	}
}

static void quit_is_cancelled_when_the_main_window_refuses(void **state)
{
	(void)state;
	/* The "outputs are active, exit anyway?" prompt answered with Cancel. */
	main_window->accept = false;
	const bool accepted = send_quit();
	if (accepted || main_window->closeEvents != 1 || !main_window->isVisible() ||
	    visible_docks() != floating_docks) {
		fprintf(stderr,
			"quit accepted %d (expected 0), main window close events %d (expected 1), "
			"main window visible %d, docks visible %d of %d\n",
			accepted, main_window->closeEvents, main_window->isVisible(), visible_docks(), floating_docks);
		fail();
	}
}

static void quit_with_a_hidden_main_window_closes_nothing_twice(void **state)
{
	(void)state;
	/* Minimized to the tray: OBS hides the main window and its docks. */
	main_window->hide();
	for (MainDock *dock : docks) {
		dock->hide();
	}
	QApplication::processEvents();
	const bool accepted = send_quit();
	if (!accepted || main_window->closeEvents != 0) {
		fprintf(stderr, "quit accepted %d, main window close events %d (expected 0)\n", accepted,
			main_window->closeEvents);
		fail();
	}
}

static void floating_dock_stays_shown_while_inactive(void **state)
{
	(void)state;
	auto *main = new QMainWindow;
	main->setCentralWidget(new QWidget);
	auto *dock = new QDockWidget(QStringLiteral("Scenes"), main);
	dock->setObjectName(QStringLiteral("scenesDock"));
	dock->setWidget(new QWidget);
	KeepFloatingDockShown(dock);
	main->addDockWidget(Qt::LeftDockWidgetArea, dock);
	main->show();
	QApplication::processEvents();
	dock->setFloating(true);
	dock->show();
	QApplication::processEvents();

	const bool floating = dock->isFloating();
	const bool tool = dock->isWindow() && (dock->windowFlags() & Qt::Tool) == Qt::Tool;
	const bool shown = dock->testAttribute(Qt::WA_MacAlwaysShowToolWindow);
	delete main;
	if (!floating || !tool || !shown) {
		fprintf(stderr, "floating %d, tool window %d, WA_MacAlwaysShowToolWindow %d (expected 1 1 1)\n",
			floating, tool, shown);
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
		cmocka_unit_test_setup_teardown(quit_closes_the_main_window_with_floating_main_docks, quit_setup,
						quit_teardown),
		cmocka_unit_test_setup_teardown(quit_is_cancelled_when_the_main_window_refuses, quit_setup,
						quit_teardown),
		cmocka_unit_test_setup_teardown(quit_with_a_hidden_main_window_closes_nothing_twice, quit_setup,
						quit_teardown),
		cmocka_unit_test(floating_dock_stays_shown_while_inactive),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
