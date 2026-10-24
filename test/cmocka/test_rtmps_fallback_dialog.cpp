/* The dialog the frontend shows when a stream that rtmp-services upgraded to
 * RTMPS fails to connect (obs-rust/obs-studio#2205). "Proceed anyway" fills a
 * progress bar for three seconds and then accepts on its own, so a stream
 * started with nobody at the screen still goes live; Cancel stops the start.
 * Each fallback is written to stderr and the OBS log. Runs the real dialog on
 * Qt's offscreen platform. */

#include <QApplication>
#include <QDialogButtonBox>
#include <QElapsedTimer>
#include <QProgressBar>
#include <QPushButton>
#include <QThread>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
extern "C" {
#include <cmocka.h>
}

#include <util/base.h>

#include "OBSRtmpsFallback.hpp"

static const OBSRtmpsFallback::Texts texts = {"Title", "Message", "Proceed anyway", "Cancel"};

/* Runs the event loop until the dialog has a result or limit_ms passes, and
 * returns how long it took. */
static qint64 wait_for_result(OBSRtmpsFallback &dlg, int limit_ms)
{
	QElapsedTimer t;
	t.start();
	while (dlg.isVisible() && t.elapsed() < limit_ms) {
		QApplication::processEvents(QEventLoop::AllEvents, 20);
		QThread::msleep(5);
	}
	return t.elapsed();
}

static QPushButton *button(OBSRtmpsFallback &dlg, QDialogButtonBox::ButtonRole role)
{
	QDialogButtonBox *box = dlg.findChild<QDialogButtonBox *>();
	if (!box) {
		return nullptr;
	}
	for (QAbstractButton *b : box->buttons()) {
		if (box->buttonRole(b) == role) {
			return qobject_cast<QPushButton *>(b);
		}
	}
	return nullptr;
}

/* With no click, the dialog accepts on its own once the countdown ends. */
static void countdown_proceeds_after_three_seconds(void **state)
{
	(void)state;
	OBSRtmpsFallback dlg(nullptr, texts);
	dlg.show();

	qint64 took = wait_for_result(dlg, 6000);

	assert_false(dlg.isVisible());
	assert_int_equal(dlg.result(), QDialog::Accepted);
	assert_true(took >= OBSRtmpsFallback::CountdownMs - 200);
	assert_true(took <= OBSRtmpsFallback::CountdownMs + 1500);
}

/* The progress bar shows how much of the countdown has passed. */
static void progress_bar_follows_countdown(void **state)
{
	(void)state;
	OBSRtmpsFallback dlg(nullptr, texts);
	dlg.show();

	wait_for_result(dlg, OBSRtmpsFallback::CountdownMs / 2);

	QProgressBar *bar = dlg.findChild<QProgressBar *>();
	assert_non_null(bar);
	int pct = bar->value() * 100 / bar->maximum();
	assert_true(pct >= 30);
	assert_true(pct <= 75);
	dlg.reject();
}

/* Clicking "Proceed anyway" accepts right away. */
static void proceed_click_accepts_now(void **state)
{
	(void)state;
	OBSRtmpsFallback dlg(nullptr, texts);
	dlg.show();

	QPushButton *proceed = button(dlg, QDialogButtonBox::AcceptRole);
	assert_non_null(proceed);
	assert_string_equal(proceed->text().toUtf8().constData(), "Proceed anyway");
	proceed->click();

	assert_true(wait_for_result(dlg, 500) < 500);
	assert_int_equal(dlg.result(), QDialog::Accepted);
}

/* Cancel rejects, and the countdown does not accept afterwards. */
static void cancel_stops_the_countdown(void **state)
{
	(void)state;
	OBSRtmpsFallback dlg(nullptr, texts);
	dlg.show();

	QPushButton *cancel = button(dlg, QDialogButtonBox::RejectRole);
	assert_non_null(cancel);
	cancel->click();
	wait_for_result(dlg, 500);
	assert_int_equal(dlg.result(), QDialog::Rejected);

	QElapsedTimer t;
	t.start();
	while (t.elapsed() < OBSRtmpsFallback::CountdownMs + 500) {
		QApplication::processEvents(QEventLoop::AllEvents, 20);
		QThread::msleep(5);
	}
	assert_int_equal(dlg.result(), QDialog::Rejected);
	assert_false(dlg.isVisible());
}

static char logged[1024];

static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)level;
	(void)param;
	char line[512];
	vsnprintf(line, sizeof(line), format, args);
	strncat(logged, line, sizeof(logged) - strlen(logged) - 1);
	strncat(logged, "\n", sizeof(logged) - strlen(logged) - 1);
}

/* A fallback is written to stderr and to the OBS log, naming the server. */
static void fallback_is_logged_to_stderr_and_log(void **state)
{
	(void)state;
	char path[] = "test_rtmps_fallback_stderr_XXXXXX";
	int fd = mkstemp(path);
	assert_true(fd >= 0);

	fflush(stderr);
	int saved = dup(fileno(stderr));
	dup2(fd, fileno(stderr));

	logged[0] = '\0';
	base_set_log_handler(capture_log, nullptr);
	OBSRtmpsFallback::LogFallback("rtmp://ingest.example/app");
	base_set_log_handler(nullptr, nullptr);

	fflush(stderr);
	dup2(saved, fileno(stderr));
	close(saved);

	char err[1024] = {0};
	lseek(fd, 0, SEEK_SET);
	ssize_t n = read(fd, err, sizeof(err) - 1);
	close(fd);
	unlink(path);

	assert_true(n > 0);
	assert_non_null(strstr(err, "RTMPS"));
	assert_non_null(strstr(err, "rtmp://ingest.example/app"));
	assert_non_null(strstr(logged, "RTMPS"));
	assert_non_null(strstr(logged, "rtmp://ingest.example/app"));
}

int main(int argc, char **argv)
{
	qputenv("QT_QPA_PLATFORM", "offscreen");
	QApplication app(argc, argv);

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(countdown_proceeds_after_three_seconds),
		cmocka_unit_test(progress_bar_follows_countdown),
		cmocka_unit_test(proceed_click_accepts_now),
		cmocka_unit_test(cancel_stops_the_countdown),
		cmocka_unit_test(fallback_is_logged_to_stderr_and_log),
	};
	return cmocka_run_group_tests(tests, nullptr, nullptr);
}
