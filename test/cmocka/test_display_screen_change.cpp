/* A preview display moved to a screen with another pixel ratio
 * (obs-rust/obs-studio#4010).
 *
 * OBSQTDisplay lays out its scene from DisplayResized: the main preview, the
 * studio-mode program view, projectors. On macOS, a window dragged to a
 * screen with another pixel ratio keeps its size in points, so Qt sends no
 * resize event, only QWindow::screenChanged; the size in pixels changes. The
 * display resized its obs_display on screenChanged but did not emit
 * DisplayResized, so the studio-mode program view kept the scale and
 * position of the old pixel ratio until the window was resized by hand.
 *
 * The test builds the real OBSQTDisplay (its own unity group) on Qt's
 * offscreen platform with two screens. A real move there is no model of
 * macOS: the offscreen platform keeps a window's size in pixels across a
 * screen change, so its size in points changes and a resize event comes
 * too, which would hide the bug. So the test sends the display's window
 * what macOS sends, a screenChanged and no resize, and expects
 * DisplayResized. FillPlatformGraphicsWindow fails here, so no obs_display
 * is made and no obs_startup() is needed. */

#include <QApplication>
#include <QByteArray>
#include <QDir>
#include <QFile>
#include <QScreen>
#include <QTemporaryDir>
#include <QWidget>
#include <QWindow>

#include "widgets/OBSQTDisplay.hpp"

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
extern "C" {
#include <cmocka.h>
}

/* The frontend's platform files are not built into this test: no native
 * window, so CreateDisplay() stops before obs_display_create(), and no native
 * events to handle. */
bool FillPlatformGraphicsWindow(QWindow *, gs_window &)
{
	return false;
}

/* As on X11 (platform-x11.cpp.hpp). */
bool PlatformResetsHiddenDisplay()
{
	return true;
}

void HandlePlatformDisplayNativeEvent(OBSQTDisplay *, void *) {}

/* Two screens side by side (a Retina next to a normal display); main() gives
 * the second pixel ratio 2. */
static const char screens_config[] = R"({
	"synchronousWindowSystemEvents": true,
	"screens": [
		{ "name": "normal", "x": 0, "y": 0, "width": 1024, "height": 768,
		  "logicalDpi": 96, "logicalBaseDpi": 96, "dpr": 1 },
		{ "name": "retina", "x": 1024, "y": 0, "width": 1024, "height": 768,
		  "logicalDpi": 96, "logicalBaseDpi": 96, "dpr": 1 }
	]
})";

static QScreen *screen_named(const char *name)
{
	for (QScreen *screen : QGuiApplication::screens()) {
		if (screen->name() == QLatin1String(name)) {
			return screen;
		}
	}
	return nullptr;
}

static void settle()
{
	for (int i = 0; i < 5; i++) {
		QApplication::processEvents();
	}
}

static void the_offscreen_platform_has_both_screens(void **)
{
	QScreen *normal = screen_named("normal");
	QScreen *retina = screen_named("retina");
	assert_non_null(normal);
	assert_non_null(retina);
	assert_true(normal->devicePixelRatio() == 1.0);
	assert_true(retina->devicePixelRatio() == 2.0);
}

static void screen_change_without_resize_reports_a_resize(void **)
{
	QWidget window;
	window.setScreen(screen_named("normal"));
	window.resize(400, 300);
	OBSQTDisplay *display = new OBSQTDisplay(&window);
	display->setGeometry(0, 0, 400, 300);
	window.show();
	settle();

	int resized = 0;
	QObject::connect(display, &OBSQTDisplay::DisplayResized, [&resized] { resized++; });
	const QSize points = display->size();

	/* What macOS sends when the window reaches the other screen. */
	emit display->windowHandle()->screenChanged(screen_named("retina"));
	settle();

	assert_true(display->size() == points);
	assert_true(resized > 0);
}

int main(int argc, char **argv)
{
	QTemporaryDir dir;
	if (!dir.isValid()) {
		return 1;
	}
	const QString config = dir.filePath("screens.json");
	QFile file(config);
	if (!file.open(QIODevice::WriteOnly) || file.write(screens_config) < 0) {
		return 1;
	}
	file.close();
	/* The offscreen platform gives a screen its "dpr" but not its windows;
	 * Qt's per-screen scale factor gives both, as on a real high-DPI
	 * desktop. */
	qputenv("QT_SCREEN_SCALE_FACTORS", "normal=1;retina=2");
	qputenv("QT_QPA_PLATFORM", QByteArray("offscreen:configfile=") + QDir::toNativeSeparators(config).toUtf8());

	QApplication app(argc, argv);

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(the_offscreen_platform_has_both_screens),
		cmocka_unit_test(screen_change_without_resize_reports_a_resize),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
