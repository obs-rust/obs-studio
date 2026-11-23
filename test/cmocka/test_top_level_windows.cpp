/* The main window and its floating docks as top-level windows: close, quit,
 * show, move and redock, one cmocka group each (obs-rust/obs-studio#5478;
 * the binary was test_close_top_level).
 *
 * Exit in the tray menu did nothing when OBS was started with
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
 * floating_dock_stays_shown_while_inactive checks that DockWindow::SetUp()
 * (frontend/utility/dock-window.hpp) sets it on a dock that then floats.
 *
 * After a floating dock was moved, the main window no longer laid out its
 * docks (obs-rust/obs-studio#5416): Qt keeps a copy of the layout during a dock
 * drag, a drag that ends outside the window leaves it set, and
 * QMainWindowLayout::setGeometry() does nothing while it is set. The moved
 * dock cases move a floating dock with its title bar, then check that a
 * resize still lays out the docks and that the dock docks into the window.
 *
 * A close of a floating main dock (not closable) did nothing but show the
 * "You just closed a dockable window" notice, and no key closed a floating
 * dock (obs-rust/obs-studio#5408). The redock cases check that
 * CloseRedocksMainDock() puts such a dock back into the main window, that a
 * quit still closes it, and that DockWindow gives a floating dock the close
 * key. dock_window_is_set_up_once checks that a dock gets one DockWindow
 * (obs-rust/obs-studio#5457). */

#include <QApplication>
#include <QCloseEvent>
#include <QDir>
#include <QDockWidget>
#include <QLibraryInfo>
#include <QMainWindow>
#include <QKeySequence>
#include <QMouseEvent>
#include <QShortcut>
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
	DockWindow::SetUp(dock);
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

/* The main window of OBS: nested and tabbed docks, no animation
 * (OBSBasic.ui), two docks with content of a real size. */
static QMainWindow *moved_main;
static QDockWidget *moved_dock;
static QDockWidget *moved_other;

static QDockWidget *add_moved_dock(const char *name, Qt::DockWidgetArea area)
{
	auto *dock = new QDockWidget(QString::fromLatin1(name), moved_main);
	dock->setObjectName(QString::fromLatin1(name));
	auto *content = new QWidget;
	content->setMinimumSize(160, 120);
	dock->setWidget(content);
	dock->setFeatures(QDockWidget::DockWidgetMovable | QDockWidget::DockWidgetFloatable);
	DockWindow::SetUp(dock);
	moved_main->addDockWidget(area, dock);
	return dock;
}

static int moved_setup(void **state)
{
	(void)state;
	moved_main = new QMainWindow;
	moved_main->setDockOptions(QMainWindow::AllowNestedDocks | QMainWindow::AllowTabbedDocks);
	auto *central = new QWidget;
	central->setMinimumSize(200, 150);
	moved_main->setCentralWidget(central);
	moved_main->resize(900, 600);
	moved_dock = add_moved_dock("scenesDock", Qt::LeftDockWidgetArea);
	moved_other = add_moved_dock("sourcesDock", Qt::LeftDockWidgetArea);
	moved_main->show();
	QApplication::processEvents();
	moved_dock->setFloating(true);
	moved_dock->show();
	QApplication::processEvents();
	return 0;
}

static int moved_teardown(void **state)
{
	(void)state;
	delete moved_main;
	moved_main = nullptr;
	moved_dock = nullptr;
	moved_other = nullptr;
	QApplication::processEvents();
	return 0;
}

/* Moves a floating dock by its window frame, as a user does on macOS, and lets
 * it go where it does not dock: a press on the frame above the dock starts
 * Qt's drag, the window moves (each move hovers the main window, which sets the
 * copy), and the drag ends without docking. macOS ends it at the release,
 * other systems at the next frame mouse move; both keep the copy. The
 * offscreen platform gives a window a frame of 2 pixels. */
static void move_floating_dock(QDockWidget *dock, QPoint by)
{
	auto send = [dock](QEvent::Type type, QPoint to, Qt::MouseButton button, Qt::MouseButtons buttons) {
		QMouseEvent event(type, dock->mapFromGlobal(QPointF(to)), QPointF(to), button, buttons, Qt::NoModifier);
		QApplication::sendEvent(dock, &event);
	};
	const QPoint frame(dock->geometry().center().x(), dock->geometry().top() - 1);
	send(QEvent::NonClientAreaMouseButtonPress, frame, Qt::LeftButton, Qt::LeftButton);
	dock->move(dock->pos() + by);
	QApplication::processEvents();
	send(QEvent::NonClientAreaMouseMove, frame + by, Qt::NoButton, Qt::LeftButton);
	send(QEvent::NonClientAreaMouseButtonRelease, frame + by, Qt::LeftButton, Qt::NoButton);
	QApplication::processEvents();
}

static void moved_report(const char *what)
{
	const QRect d = moved_dock->geometry();
	const QRect o = moved_other->geometry();
	const QRect c = moved_main->centralWidget()->geometry();
	fprintf(stderr,
		"%s: dock floating %d visible %d at %d,%d %dx%d; other dock at %d,%d %dx%d; central at %d,%d %dx%d; "
		"main window %dx%d\n",
		what, moved_dock->isFloating(), moved_dock->isVisible(), d.x(), d.y(), d.width(), d.height(), o.x(),
		o.y(), o.width(), o.height(), c.x(), c.y(), c.width(), c.height(), moved_main->width(),
		moved_main->height());
}

static void moved_dock_keeps_the_main_window_layout(void **state)
{
	(void)state;
	move_floating_dock(moved_dock, QPoint(1000, 100));
	const int before = moved_main->centralWidget()->width();
	moved_main->resize(moved_main->width() + 60, moved_main->height());
	QApplication::processEvents();
	const int after = moved_main->centralWidget()->width();
	if (!moved_dock->isFloating() || after != before + 60) {
		moved_report("resize after a move");
		fprintf(stderr, "central width %d before, %d after the resize (expected %d)\n", before, after,
			before + 60);
		fail();
	}
}

static void moved_dock_docks_into_the_main_window(void **state)
{
	(void)state;
	move_floating_dock(moved_dock, QPoint(1000, 100));
	/* What a double-click on the title bar does. */
	moved_dock->setFloating(false);
	QApplication::processEvents();
	const QRect d = moved_dock->geometry();
	if (moved_dock->isFloating() || !moved_dock->isVisible() || d.width() < 160 || d.left() < 0 || d.top() < 0 ||
	    d.right() >= moved_main->width() || d.bottom() >= moved_main->height()) {
		moved_report("dock docked after a move");
		fail();
	}
}

/* OBSDock::closeEvent() without the frontend: `closing` is OBSBasic's
 * isClosing(). */
class RedockDock : public QDockWidget {
public:
	RedockDock(const QString &title, QMainWindow *main) : QDockWidget(title, main) {}
	bool closing = false;
	int closeEvents = 0;

protected:
	void closeEvent(QCloseEvent *event) override
	{
		closeEvents++;
		if (CloseRedocksMainDock(this, event, closing)) {
			return;
		}
		QDockWidget::closeEvent(event);
	}
};

static QMainWindow *redock_main;
static RedockDock *redock_dock;
static QShortcut *redock_key;

/* A main window with one dock that is set up as OBS sets up its docks, then
 * floated. */
static void float_one_dock(bool closable)
{
	redock_main = new QMainWindow;
	redock_main->setCentralWidget(new QWidget);
	redock_main->resize(800, 600);
	redock_dock = new RedockDock(QStringLiteral("Scenes"), redock_main);
	redock_dock->setObjectName(QStringLiteral("scenesDock"));
	redock_dock->setWidget(new QWidget);
	redock_dock->setFeatures(QDockWidget::DockWidgetMovable | QDockWidget::DockWidgetFloatable |
				 (closable ? QDockWidget::DockWidgetClosable : QDockWidget::NoDockWidgetFeatures));
	redock_key = DockWindow::SetUp(redock_dock)->CloseKey();
	redock_main->addDockWidget(Qt::LeftDockWidgetArea, redock_dock);
	redock_main->show();
	QApplication::processEvents();
	redock_dock->setFloating(true);
	redock_dock->show();
	QApplication::processEvents();
}

static int redock_setup(void **state)
{
	(void)state;
	float_one_dock(false);
	return 0;
}

static int redock_closable_setup(void **state)
{
	(void)state;
	float_one_dock(true);
	return 0;
}

static int redock_teardown(void **state)
{
	(void)state;
	delete redock_main;
	redock_main = nullptr;
	redock_dock = nullptr;
	redock_key = nullptr;
	QApplication::processEvents();
	return 0;
}

static void report_dock(const char *what, bool floating, bool visible)
{
	fprintf(stderr, "%s: floating %d (expected %d), visible %d (expected %d), close events %d\n", what,
		redock_dock->isFloating(), floating, redock_dock->isVisible(), visible, redock_dock->closeEvents);
}

static void close_of_a_floating_main_dock_puts_it_back(void **state)
{
	(void)state;
	redock_dock->close();
	QApplication::processEvents();
	if (redock_dock->isFloating() || !redock_dock->isVisible() || redock_dock->closeEvents != 1) {
		report_dock("close of a floating main dock", false, true);
		fail();
	}
}

static void quit_closes_a_floating_main_dock(void **state)
{
	(void)state;
	/* Since #5406 a quit closes the main window first. */
	redock_main->hide();
	QApplication::processEvents();
	redock_dock->close();
	QApplication::processEvents();
	if (!redock_dock->isFloating() || redock_dock->isVisible()) {
		report_dock("close during a quit", true, false);
		fail();
	}
}

static void close_while_obs_closes_keeps_the_dock_floating(void **state)
{
	(void)state;
	redock_dock->closing = true;
	redock_dock->close();
	QApplication::processEvents();
	if (!redock_dock->isFloating()) {
		report_dock("close while OBS closes", true, redock_dock->isVisible());
		fail();
	}
}

static void close_of_a_closable_floating_dock_hides_it(void **state)
{
	(void)state;
	redock_dock->close();
	QApplication::processEvents();
	if (redock_dock->isVisible()) {
		report_dock("close of a closable floating dock", true, false);
		fail();
	}
}

static void close_key_puts_a_floating_main_dock_back(void **state)
{
	(void)state;
	if (!redock_key) {
		fprintf(stderr, "DockWindow gave no close key\n");
		fail();
	}
	const bool keys = redock_key->keys() == QKeySequence::keyBindings(QKeySequence::Close);
	const bool own = redock_key->parent() == redock_dock && redock_key->context() == Qt::WindowShortcut;
	const bool enabled_floating = redock_key->isEnabled();
	emit redock_key->activated();
	QApplication::processEvents();
	const bool enabled_docked = redock_key->isEnabled();
	if (!keys || !own || !enabled_floating || enabled_docked || redock_dock->isFloating() ||
	    !redock_dock->isVisible()) {
		fprintf(stderr,
			"close keys %d, dock window shortcut %d, enabled while floating %d, enabled while docked %d "
			"(expected 1 1 1 0)\n",
			keys, own, enabled_floating, enabled_docked);
		report_dock("close key on a floating main dock", false, true);
		fail();
	}
}

static void close_key_is_added_once(void **state)
{
	(void)state;
	/* An OBSDock is set up when it is made and again in AddDockWidget(). Two
	 * shortcuts on one key are ambiguous, and Qt activates neither. */
	QShortcut *again = DockWindow::SetUp(redock_dock)->CloseKey();
	const qsizetype keys = redock_dock->findChildren<QShortcut *>(Qt::FindDirectChildrenOnly).size();
	if (!redock_key || again != redock_key || keys != 1) {
		fprintf(stderr, "first key %p, second call %p, shortcuts on the dock %d (expected the same one, 1)\n",
			(void *)redock_key, (void *)again, (int)keys);
		fail();
	}
}

/* #5457: what OBS adds to a dock window (the macOS attribute, the move-end
 * filter, the close key) is one DockWindow object. An OBSDock is set up when
 * it is made and again in AddDockWidget(), so a second SetUp() must give the
 * same object: one filter and one close key (two shortcuts on one key are
 * ambiguous, and Qt then activates neither). */
static void dock_window_is_set_up_once(void **state)
{
	(void)state;
	auto *main = new QMainWindow;
	main->setCentralWidget(new QWidget);
	auto *dock = new QDockWidget(QStringLiteral("Scenes"), main);
	dock->setObjectName(QStringLiteral("scenesDock"));
	dock->setWidget(new QWidget);
	main->addDockWidget(Qt::LeftDockWidgetArea, dock);
	DockWindow *first = DockWindow::SetUp(dock);
	DockWindow *second = DockWindow::SetUp(dock);
	const qsizetype windows =
		dock->findChildren<QObject *>(QStringLiteral("dockWindow"), Qt::FindDirectChildrenOnly).size();
	const qsizetype keys = dock->findChildren<QShortcut *>(Qt::FindDirectChildrenOnly).size();
	QShortcut *key = first ? first->CloseKey() : nullptr;
	const bool close_keys = key && key->keys() == QKeySequence::keyBindings(QKeySequence::Close);
	const bool shown = dock->testAttribute(Qt::WA_MacAlwaysShowToolWindow);
	delete main;
	if (!first || first != second || windows != 1 || keys != 1 || !close_keys || !shown) {
		fprintf(stderr,
			"first %p, second %p, DockWindow children %d, shortcuts %d, close keys %d, "
			"WA_MacAlwaysShowToolWindow %d (expected the same object, 1, 1, 1, 1)\n",
			(void *)first, (void *)second, (int)windows, (int)keys, close_keys, shown);
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

	/* One group per topic (obs-rust/obs-studio#5478): cmocka prints each
	 * group's name with its results. */
	const struct CMUnitTest close_tests[] = {
		cmocka_unit_test_setup_teardown(never_shown_window_gets_close_event, setup, teardown),
		cmocka_unit_test_setup_teardown(never_shown_window_can_refuse, setup, teardown),
		cmocka_unit_test_setup_teardown(shown_window_closes, setup, teardown),
		cmocka_unit_test_setup_teardown(shown_window_can_refuse, setup, teardown),
		cmocka_unit_test_setup_teardown(window_without_qwindow_closes, setup, teardown),
	};
	const struct CMUnitTest quit_tests[] = {
		cmocka_unit_test_setup_teardown(quit_closes_the_main_window_with_floating_main_docks, quit_setup,
						quit_teardown),
		cmocka_unit_test_setup_teardown(quit_is_cancelled_when_the_main_window_refuses, quit_setup,
						quit_teardown),
		cmocka_unit_test_setup_teardown(quit_with_a_hidden_main_window_closes_nothing_twice, quit_setup,
						quit_teardown),
	};
	const struct CMUnitTest dock_window_tests[] = {
		cmocka_unit_test(floating_dock_stays_shown_while_inactive),
		cmocka_unit_test(dock_window_is_set_up_once),
	};
	const struct CMUnitTest moved_dock_tests[] = {
		cmocka_unit_test_setup_teardown(moved_dock_keeps_the_main_window_layout, moved_setup, moved_teardown),
		cmocka_unit_test_setup_teardown(moved_dock_docks_into_the_main_window, moved_setup, moved_teardown),
	};
	const struct CMUnitTest redock_tests[] = {
		cmocka_unit_test_setup_teardown(close_of_a_floating_main_dock_puts_it_back, redock_setup,
						redock_teardown),
		cmocka_unit_test_setup_teardown(quit_closes_a_floating_main_dock, redock_setup, redock_teardown),
		cmocka_unit_test_setup_teardown(close_while_obs_closes_keeps_the_dock_floating, redock_setup,
						redock_teardown),
		cmocka_unit_test_setup_teardown(close_of_a_closable_floating_dock_hides_it, redock_closable_setup,
						redock_teardown),
		cmocka_unit_test_setup_teardown(close_key_puts_a_floating_main_dock_back, redock_setup,
						redock_teardown),
		cmocka_unit_test_setup_teardown(close_key_is_added_once, redock_setup, redock_teardown),
	};
	int failed = 0;
	failed += cmocka_run_group_tests_name("close", close_tests, NULL, NULL);
	failed += cmocka_run_group_tests_name("quit", quit_tests, NULL, NULL);
	failed += cmocka_run_group_tests_name("dock_window", dock_window_tests, NULL, NULL);
	failed += cmocka_run_group_tests_name("moved_dock", moved_dock_tests, NULL, NULL);
	failed += cmocka_run_group_tests_name("redock", redock_tests, NULL, NULL);
	return failed;
}
