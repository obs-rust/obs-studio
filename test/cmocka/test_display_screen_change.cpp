/* A preview display moved to a screen with another pixel ratio
 * (obs-rust/obs-studio#4010).
 *
 * OBSQTDisplay lays out its scene from DisplayResized: the main preview, the
 * studio-mode program view, projectors. When the window moves to a screen
 * with another device pixel ratio, its size in points stays the same, so Qt
 * sends no resize event, but the size in pixels changes. The display resized
 * its obs_display on QWindow::screenChanged but did not emit DisplayResized,
 * so the studio-mode program view kept the scale and position of the old
 * pixel ratio until the window was resized by hand.
 *
 * The test builds the real OBSQTDisplay (its own unity group) on Qt's
 * offscreen platform with two screens, pixel ratio 1 and 2, moves the
 * window from one to the other without a resize, and expects DisplayResized.
 * FillPlatformGraphicsWindow fails here, so no obs_display is made and no
 * obs_startup() is needed. */

#include <QApplication>
#include <QByteArray>
#include <QDir>
#include <QFile>
#include <QScreen>
#include <QTemporaryDir>
#include <QWidget>
#include <QWindow>

#include "widgets/OBSQTDisplay.hpp"
#include "utility/display-helpers.hpp"

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

/* Two screens side by side, the second with pixel ratio 2 (a Retina next to
 * a normal display). */
static const char screens_config[] = R"({
	"synchronousWindowSystemEvents": true,
	"screens": [
		{ "name": "normal", "x": 0, "y": 0, "width": 1024, "height": 768,
		  "logicalDpi": 96, "logicalBaseDpi": 96, "dpr": 1 },
		{ "name": "retina", "x": 1024, "y": 0, "width": 1024, "height": 768,
		  "logicalDpi": 96, "logicalBaseDpi": 96, "dpr": 2 }
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

static void move_to(QWidget &window, QScreen *screen)
{
	window.windowHandle()->setScreen(screen);
	window.move(screen->geometry().topLeft() + QPoint(50, 50));
	settle();
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

static void screen_change_reports_a_resize(void **)
{
	QWidget window;
	window.resize(400, 300);
	OBSQTDisplay *display = new OBSQTDisplay(&window);
	display->setGeometry(0, 0, 400, 300);

	move_to(window, screen_named("normal"));
	window.show();
	settle();

	int resized = 0;
	QObject::connect(display, &OBSQTDisplay::DisplayResized, [&resized] { resized++; });
	const QSize points = display->size();
	const QSize before = GetPixelSize(display);

	move_to(window, screen_named("retina"));

	/* The move changed the pixel size, not the size in points: Qt sent no
	 * resize event, so only the screen change can report it. */
	assert_true(display->size() == points);
	assert_true(GetPixelSize(display) == before * 2);
	assert_true(resized > 0);

	resized = 0;
	move_to(window, screen_named("normal"));
	assert_true(GetPixelSize(display) == before);
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
	qputenv("QT_QPA_PLATFORM", QByteArray("offscreen:configfile=") + QDir::toNativeSeparators(config).toUtf8());

	QApplication app(argc, argv);

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(the_offscreen_platform_has_both_screens),
		cmocka_unit_test(screen_change_reports_a_resize),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
