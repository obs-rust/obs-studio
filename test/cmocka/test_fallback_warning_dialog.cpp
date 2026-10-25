/* The shared fallback warning window (obs-rust/obs-studio#2218). A feature
 * that falls back to a less safe mode (the browser source without its
 * sandbox, #1092; RTMPS to RTMP, #2205) shows it: the caller's texts, a
 * countdown that accepts by itself, "Accept anyway" and, when the caller can
 * stop, "Cancel". The glue maps the end to the C ABI of
 * rust/obs-fallback-warning, so every way out must report exactly one end,
 * and the right one.
 *
 * Builds the real OBSFallbackWarning on Qt's offscreen platform, without the
 * frontend (the window takes all texts from its caller), and drives it with
 * Qt Test: the countdown, both buttons, Escape and the window close. */

#include <QApplication>
#include <QDialog>
#include <QLabel>
#include <QPointer>
#include <QProgressBar>
#include <QPushButton>
#include <QTest>

#include "dialogs/OBSFallbackWarning.hpp"

#include <vector>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
extern "C" {
#include <cmocka.h>
}

using End = OBSFallbackWarning::End;

static OBSFallbackWarning::Texts texts(bool can_cancel)
{
	OBSFallbackWarning::Texts t;
	t.title = "Browser source without sandbox";
	t.message = "The Chromium sandbox could not start.";
	t.futureError = "This may become an error in a future version of OBS.";
	t.accept = "Accept anyway";
	t.cancel = can_cancel ? "Cancel" : QString();
	return t;
}

struct Shown {
	/* The context of the ended() connection: it goes with this struct, so a
	 * dialog that a later teardown closes cannot write into a finished test. */
	QObject context;
	QPointer<OBSFallbackWarning> dialog;
	std::vector<End> ends;
};

static void show(Shown &s, bool can_cancel, qint64 countdown_ms)
{
	s.dialog = new OBSFallbackWarning(nullptr, texts(can_cancel), countdown_ms);
	QObject::connect(s.dialog.data(), &OBSFallbackWarning::ended, &s.context,
			 [&s](End end) { s.ends.push_back(end); });
	s.dialog->show();
	QTest::qWait(20);
}

/* Lets deferred deletes run, so a dialog that deletes itself on close is gone. */
static void settle(int ms)
{
	QTest::qWait(ms);
	QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
	QCoreApplication::processEvents();
}

static void check_one_end(Shown &s, End expected)
{
	assert_int_equal(s.ends.size(), 1);
	assert_int_equal((int)s.ends[0], (int)expected);
	/* The window closes and deletes itself: nobody holds it after the end. */
	assert_null(s.dialog.data());
}

static void click(Shown &s, const char *name)
{
	auto *button = s.dialog->findChild<QPushButton *>(name);
	assert_non_null(button);
	QTest::mouseClick(button, Qt::LeftButton);
}

static int teardown(void **)
{
	QApplication::closeAllWindows();
	settle(0);
	return 0;
}

static void shows_the_callers_texts(void **)
{
	Shown s;
	show(s, true, 60000);

	assert_true(s.dialog->windowTitle() == "Browser source without sandbox");
	auto *message = s.dialog->findChild<QLabel *>("message");
	auto *future = s.dialog->findChild<QLabel *>("futureError");
	auto *accept = s.dialog->findChild<QPushButton *>("acceptButton");
	auto *cancel = s.dialog->findChild<QPushButton *>("cancelButton");
	assert_non_null(message);
	assert_non_null(future);
	assert_non_null(accept);
	assert_non_null(cancel);
	assert_true(message->text() == "The Chromium sandbox could not start.");
	assert_true(future->text() == "This may become an error in a future version of OBS.");
	assert_true(accept->text() == "Accept anyway");
	assert_true(cancel->text() == "Cancel");
	assert_true(cancel->isVisible());
	/* Not modal: OBS keeps running while it counts down. */
	assert_false(s.dialog->isModal());
	assert_true(s.ends.empty());
}

static void no_cancel_button_without_cancel_text(void **)
{
	Shown s;
	show(s, false, 60000);

	auto *accept = s.dialog->findChild<QPushButton *>("acceptButton");
	auto *cancel = s.dialog->findChild<QPushButton *>("cancelButton");
	assert_non_null(accept);
	assert_true(accept->isVisible());
	assert_true(cancel == nullptr || !cancel->isVisible());
}

static void countdown_moves_the_progress(void **)
{
	Shown s;
	show(s, true, 2000);
	settle(400);

	auto *progress = s.dialog->findChild<QProgressBar *>("progress");
	assert_non_null(progress);
	assert_true(s.dialog->elapsedShown() > 0);
	assert_true(s.dialog->elapsedShown() < 2000);
	assert_int_equal(progress->maximum(), 2000);
	assert_int_equal(progress->value(), s.dialog->elapsedShown());
	assert_true(s.ends.empty());
}

static void countdown_accepts_once(void **)
{
	Shown s;
	show(s, true, 200);
	settle(700);

	check_one_end(s, End::CountdownDone);
}

static void accept_click_ends_once(void **)
{
	Shown s;
	show(s, true, 300);
	click(s, "acceptButton");
	/* Past the countdown: a stopped countdown must not end it again. */
	settle(700);

	check_one_end(s, End::AcceptClicked);
}

static void cancel_click_ends_once(void **)
{
	Shown s;
	show(s, true, 300);
	click(s, "cancelButton");
	settle(700);

	check_one_end(s, End::CancelClicked);
}

static void escape_closes(void **)
{
	Shown s;
	show(s, true, 300);
	QTest::keyClick(s.dialog.data(), Qt::Key_Escape);
	settle(700);

	check_one_end(s, End::Closed);
}

/* Another modal dialog opened over the warning (OBS queues the first-run
 * wizard right after the main window has loaded): the warning is hidden or
 * takes no input, so its countdown must not accept it unseen. It pauses
 * while that dialog is open and goes on after it has closed. */
static void countdown_pauses_while_another_dialog_is_modal(void **)
{
	Shown s;
	show(s, true, 400);
	settle(100);

	QDialog *modal = new QDialog();
	modal->setModal(true);
	modal->show();
	settle(50);
	assert_true(QApplication::activeModalWidget() == modal);
	qint64 before = s.dialog->elapsedShown();
	settle(900);
	assert_true(s.ends.empty());
	assert_non_null(s.dialog.data());
	/* One tick may still have counted when the modal opened. */
	assert_true(s.dialog->elapsedShown() <= before + 100);

	modal->close();
	delete modal;
	settle(900);
	check_one_end(s, End::CountdownDone);
}

static void window_close_closes(void **)
{
	Shown s;
	show(s, false, 300);
	s.dialog->close();
	settle(700);

	check_one_end(s, End::Closed);
}

int main(int argc, char **argv)
{
	qputenv("QT_QPA_PLATFORM", "offscreen");
	QApplication app(argc, argv);

	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(shows_the_callers_texts, teardown),
		cmocka_unit_test_teardown(no_cancel_button_without_cancel_text, teardown),
		cmocka_unit_test_teardown(countdown_moves_the_progress, teardown),
		cmocka_unit_test_teardown(countdown_accepts_once, teardown),
		cmocka_unit_test_teardown(accept_click_ends_once, teardown),
		cmocka_unit_test_teardown(cancel_click_ends_once, teardown),
		cmocka_unit_test_teardown(escape_closes, teardown),
		cmocka_unit_test_teardown(window_close_closes, teardown),
		cmocka_unit_test_teardown(countdown_pauses_while_another_dialog_is_modal, teardown),
	};
	if (argc > 1) {
		cmocka_set_test_filter(argv[1]);
	}
	return cmocka_run_group_tests(tests, NULL, NULL);
}
