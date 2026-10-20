/* Help > Crash Reports > Upload Last Crash Report with no crash log to
 * upload (upstream obsproject/obs-studio#13401): the upload dialog stays in
 * its "Uploading..." state for good. The dialog waits for the crash handler's
 * crashLogUploadFinished or crashLogUploadFailed signal, and
 * CrashHandler::uploadCrashLogToServer() returned without either when the
 * crash log was missing, empty or unreadable. On a system that never had a
 * crash there is no crash log at all, so the first upload already hangs.
 * Linux keeps no crash logs and has no Crash Reports menu; Windows and macOS
 * do.
 *
 * The test builds the real utility/CrashHandler.cpp (and its Linux part,
 * which finds no crash log) against stand-ins for the frontend app
 * (crash-handler-stubs/), asks it to upload the last crash log, and expects
 * it to report a failure. QtCore only; no display, no network. */

// The C++ and Qt headers go first: cmocka.h defines macros (fail, ...) that
// clash with them.
#include "frontend/utility/CrashHandler.hpp"

#include <OBSApp.hpp>

#include <QCoreApplication>
#include <QString>

#include <filesystem>
#include <string>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
// The Windows cmocka.h has no extern "C" guard of its own.
extern "C" {
#include <cmocka.h>
}

#include <util/bmem.h>

namespace {
OBSApp app;
}

OBSApp *App()
{
	return &app;
}

char *GetAppConfigPathPtr(const char *name)
{
	std::string path = (std::filesystem::current_path() / "crash_log_upload_test" / name).u8string();
	return bstrdup(path.c_str());
}

bool ensure_crash_sentinel_dir(const std::filesystem::path &)
{
	return false;
}

/* The upload never starts here: no crash log. */
void RemoteTextThread::run() {}

static void upload_without_crash_log_reports_failure(void **state)
{
	UNUSED_PARAMETER(state);

	OBS::CrashHandler handler;
	int started = 0;
	int finished = 0;
	int failed = 0;
	QString failure;

	QObject::connect(&handler, &OBS::CrashHandler::crashLogUploadStarted, [&]() { started++; });
	QObject::connect(&handler, &OBS::CrashHandler::crashLogUploadFinished, [&](const QString &) { finished++; });
	QObject::connect(&handler, &OBS::CrashHandler::crashLogUploadFailed, [&](const QString &error) {
		failed++;
		failure = error;
	});

	handler.uploadLastCrashLog();

	assert_int_equal(started, 0);
	assert_int_equal(finished, 0);
	assert_int_equal(failed, 1);
	assert_string_equal(failure.toUtf8().constData(), "LogUploadDialog.Errors.NoLogFile");
}

int main(int argc, char **argv)
{
	QCoreApplication qapp(argc, argv);

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(upload_without_crash_log_reports_failure),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
