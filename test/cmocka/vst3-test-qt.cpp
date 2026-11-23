#include <QApplication>
#include <QEvent>
#include <QWidget>

// Use the real platform integration when exercising the VST3 editor.
extern "C" void *vst3_test_qt_start(int *argc, char **argv, bool editor)
{
	if (editor) {
		return static_cast<QCoreApplication *>(new QApplication(*argc, argv));
	}
	return new QCoreApplication(*argc, argv);
}

extern "C" void vst3_test_qt_drain(void)
{
	QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
}

extern "C" void vst3_test_qt_stop(void *app)
{
	delete static_cast<QCoreApplication *>(app);
}

static QWidget *find_editor()
{
	QCoreApplication::processEvents();
	for (QWidget *widget : QApplication::topLevelWidgets()) {
		if (widget->windowTitle().startsWith(QStringLiteral("Gain: VST3 Plugin - "))) {
			return widget;
		}
	}
	return nullptr;
}

extern "C" bool vst3_test_qt_window_visible()
{
	QWidget *widget = find_editor();
	return widget && widget->isVisible();
}

extern "C" bool vst3_test_qt_close_window()
{
	QWidget *widget = find_editor();
	if (!widget) {
		return false;
	}
	widget->close();
	return true;
}
