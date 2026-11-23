#include <QCoreApplication>
#include <QEvent>

// Keep test assertions on the public C API; Qt is only the app lifetime stub.
extern "C" void *vst3_test_qt_start(int *argc, char **argv)
{
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
