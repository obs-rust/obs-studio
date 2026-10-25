/* The shared fallback warning between its callers and the window
 * (obs-rust/obs-studio#2218, frontend/utility/FallbackWarning). A caller on
 * any thread warns; the glue logs at error level at once, then on the UI
 * thread shows OBSFallbackWarning or accepts, as rust/obs-fallback-warning
 * decides, and calls the caller back once. Warnings wait for the main window
 * (a browser source falls back while the scene collection loads), and only
 * one dialog shows at a time.
 *
 * Builds the glue and the window on Qt's offscreen platform with the
 * frontend's real Rust staticlib (rust/obs-frontend) when it is built, and
 * captures the log through its own blog(). Without the frontend's Rust (the
 * C rollback build) the glue only logs and accepts, and that is checked
 * instead. The Rust session and the glue are per process: every case uses
 * its own ids and ends with the shutdown call, and the case before any main
 * window runs first. */

#include <QApplication>
#include <QDialog>
#include <QPushButton>
#include <QTest>
#include <QThread>

#include "dialogs/OBSFallbackWarning.hpp"
#include "utility/FallbackWarning.hpp"

#include <util/base.h>

#include <stdarg.h>
#include <stdio.h>
#include <string>
#include <thread>
#include <list>
#include <vector>

#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
extern "C" {
#include <cmocka.h>
}

struct LogLine {
	int level;
	std::string text;
};
static std::vector<LogLine> logged;

extern "C" void blog(int log_level, const char *format, ...)
{
	char buf[1024];
	va_list args;
	va_start(args, format);
	vsnprintf(buf, sizeof(buf), format, args);
	va_end(args);
	logged.push_back({log_level, buf});
}

struct Answer {
	bool accepted;
	bool on_ui_thread;
};

struct Caller {
	std::vector<Answer> answers;
};

/* Every case's callers live until the next case: a failed assertion leaves
 * its case early, and the teardown's shutdown call still answers them. */
static std::list<Caller> callers;

static Caller &new_caller()
{
	return callers.emplace_back();
}

static void answer(void *param, bool accepted)
{
	auto *caller = static_cast<Caller *>(param);
	caller->answers.push_back({accepted, QThread::currentThread() == qApp->thread()});
}

static FallbackWarning::Texts texts()
{
	FallbackWarning::Texts t;
	t.accept = "Accept anyway";
	t.cancel = "Cancel";
	t.futureError = "This may become an error in a future version of OBS.";
	return t;
}

static QWidget *main_window;

static void settle(int ms = 30)
{
	QTest::qWait(ms);
	QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
	QCoreApplication::processEvents();
}

static std::vector<OBSFallbackWarning *> dialogs()
{
	std::vector<OBSFallbackWarning *> shown;
	if (main_window) {
		for (OBSFallbackWarning *d : main_window->findChildren<OBSFallbackWarning *>()) {
			if (d->isVisible()) {
				shown.push_back(d);
			}
		}
	}
	return shown;
}

static void click(OBSFallbackWarning *dialog, const char *name)
{
	auto *button = dialog->findChild<QPushButton *>(name);
	assert_non_null(button);
	QTest::mouseClick(button, Qt::LeftButton);
	settle();
}

static int setup(void **)
{
	logged.clear();
	callers.clear();
	main_window = new QWidget();
	main_window->show();
	settle();
	return 0;
}

static int teardown(void **)
{
	FallbackWarning::SetMainWindow(nullptr, texts());
	settle();
	delete main_window;
	main_window = nullptr;
	settle();
	return 0;
}

/* Runs first: the glue has never seen a main window. The log line is
 * written at once, at error level, on the caller's thread. Before the main
 * window has loaded nothing shows and nobody is answered; then the warning
 * runs, on the UI thread. */
static void logs_at_once_then_waits_for_the_main_window(void **)
{
	Caller &caller = new_caller();
	std::thread t([&caller]() {
		FallbackWarning::Warn("test.wait", "Wait title", "The sandbox could not start.", true, answer, &caller);
	});
	t.join();

	assert_int_equal(logged.size(), 1);
	assert_int_equal(logged[0].level, LOG_ERROR);
	assert_string_equal(logged[0].text.c_str(), "[fallback] test.wait: The sandbox could not start. "
						    "This may become an error in a future version of OBS.");
	settle();
	assert_true(caller.answers.empty());
	assert_int_equal(dialogs().size(), 0);

	FallbackWarning::SetMainWindow(main_window, texts());
	settle();
#ifdef OBS_FRONTEND_RUST
	assert_int_equal(dialogs().size(), 1);
	OBSFallbackWarning *dialog = dialogs()[0];
	assert_true(dialog->windowTitle() == "Wait title");
	assert_true(caller.answers.empty());
	click(dialog, "cancelButton");
	assert_int_equal(caller.answers.size(), 1);
	assert_false(caller.answers[0].accepted);
#else
	assert_int_equal(dialogs().size(), 0);
	assert_int_equal(caller.answers.size(), 1);
	assert_true(caller.answers[0].accepted);
#endif
	assert_true(caller.answers[0].on_ui_thread);
}

/* Never inside Warn(), even on the UI thread with the main window ready. */
static void never_answers_inside_warn(void **)
{
	FallbackWarning::SetMainWindow(main_window, texts());
	Caller &caller = new_caller();
	FallbackWarning::Warn("test.inside", "Title", "Message", false, answer, &caller);
	assert_true(caller.answers.empty());
	settle();
#ifdef OBS_FRONTEND_RUST
	assert_int_equal(dialogs().size(), 1);
	click(dialogs()[0], "acceptButton");
#endif
	assert_int_equal(caller.answers.size(), 1);
	assert_true(caller.answers[0].accepted);
	assert_true(caller.answers[0].on_ui_thread);
}

/* A caller that cannot stop gets no Cancel button. */
static void no_cancel_for_a_caller_that_cannot_stop(void **)
{
	FallbackWarning::SetMainWindow(main_window, texts());
	Caller &caller = new_caller();
	FallbackWarning::Warn("test.nocancel", "Title", "Message", false, answer, &caller);
	settle();
#ifdef OBS_FRONTEND_RUST
	assert_int_equal(dialogs().size(), 1);
	auto *cancel = dialogs()[0]->findChild<QPushButton *>("cancelButton");
	assert_true(cancel == nullptr || !cancel->isVisible());
	click(dialogs()[0], "acceptButton");
#endif
	assert_int_equal(caller.answers.size(), 1);
	assert_true(caller.answers[0].accepted);
}

/* One dialog at a time: the second waits for the first. */
static void one_dialog_at_a_time(void **)
{
	FallbackWarning::SetMainWindow(main_window, texts());
	Caller &first = new_caller();
	Caller &second = new_caller();
	FallbackWarning::Warn("test.first", "First", "Message", true, answer, &first);
	FallbackWarning::Warn("test.second", "Second", "Message", true, answer, &second);
	settle();
#ifdef OBS_FRONTEND_RUST
	assert_int_equal(dialogs().size(), 1);
	assert_true(dialogs()[0]->windowTitle() == "First");
	click(dialogs()[0], "acceptButton");
	assert_int_equal(first.answers.size(), 1);
	assert_true(second.answers.empty());

	assert_int_equal(dialogs().size(), 1);
	assert_true(dialogs()[0]->windowTitle() == "Second");
	click(dialogs()[0], "cancelButton");
	assert_int_equal(second.answers.size(), 1);
	assert_false(second.answers[0].accepted);
#else
	assert_int_equal(dialogs().size(), 0);
	assert_int_equal(first.answers.size(), 1);
	assert_int_equal(second.answers.size(), 1);
#endif
	assert_true(first.answers[0].accepted);
}

/* The same id shows its dialog once per session; again, it accepts at once
 * (and still logs). */
static void same_id_shows_once(void **)
{
	FallbackWarning::SetMainWindow(main_window, texts());
	Caller &first = new_caller();
	Caller &again = new_caller();
	FallbackWarning::Warn("test.once", "Once", "Message", true, answer, &first);
	settle();
#ifdef OBS_FRONTEND_RUST
	assert_int_equal(dialogs().size(), 1);
	click(dialogs()[0], "acceptButton");
#endif
	FallbackWarning::Warn("test.once", "Once", "Message", true, answer, &again);
	settle();
	assert_int_equal(dialogs().size(), 0);
	assert_int_equal(again.answers.size(), 1);
	assert_true(again.answers[0].accepted);
	assert_int_equal(logged.size(), 2);
}

/* Nobody sees a dialog on a hidden main window: it accepts at once. */
static void hidden_main_window_accepts(void **)
{
	main_window->hide();
	FallbackWarning::SetMainWindow(main_window, texts());
	Caller &caller = new_caller();
	FallbackWarning::Warn("test.hidden", "Title", "Message", true, answer, &caller);
	settle();
	assert_int_equal(main_window->findChildren<OBSFallbackWarning *>().size(), 0);
	assert_int_equal(caller.answers.size(), 1);
	assert_true(caller.answers[0].accepted);
}

/* Another modal dialog is open (the first-run wizard opens right after the
 * main window has loaded): a warning would show behind it or take no input,
 * and accept unseen. It waits until that dialog has closed, and is not yet
 * counted as shown. */
static void waits_while_another_dialog_is_modal(void **)
{
	FallbackWarning::SetMainWindow(main_window, texts());
	QDialog *modal = new QDialog(main_window);
	modal->setModal(true);
	modal->show();
	settle();
	assert_true(QApplication::activeModalWidget() == modal);

	Caller &caller = new_caller();
	FallbackWarning::Warn("test.modal", "Behind a modal", "Message", true, answer, &caller);
	settle(400);
#ifdef OBS_FRONTEND_RUST
	assert_int_equal(dialogs().size(), 0);
	assert_true(caller.answers.empty());

	modal->close();
	settle(600);
	assert_int_equal(dialogs().size(), 1);
	assert_true(dialogs()[0]->windowTitle() == "Behind a modal");
	click(dialogs()[0], "cancelButton");
	assert_int_equal(caller.answers.size(), 1);
	assert_false(caller.answers[0].accepted);
#else
	/* No dialog to hide: it accepts. */
	assert_int_equal(caller.answers.size(), 1);
	assert_true(caller.answers[0].accepted);
	modal->close();
#endif
	delete modal;
}

/* At shutdown the open dialog closes (Close: a caller that can stop stops),
 * and queued warnings accept without a dialog. */
static void shutdown_answers_everyone(void **)
{
	FallbackWarning::SetMainWindow(main_window, texts());
	Caller &open = new_caller();
	Caller &queued = new_caller();
	FallbackWarning::Warn("test.open", "Open", "Message", true, answer, &open);
	FallbackWarning::Warn("test.queued", "Queued", "Message", true, answer, &queued);
	settle();

	FallbackWarning::SetMainWindow(nullptr, texts());
	settle();
	assert_int_equal(dialogs().size(), 0);
	assert_int_equal(open.answers.size(), 1);
	assert_int_equal(queued.answers.size(), 1);
#ifdef OBS_FRONTEND_RUST
	assert_false(open.answers[0].accepted);
#else
	assert_true(open.answers[0].accepted);
#endif
	assert_true(queued.answers[0].accepted);
}

/* No callback and NULL strings are allowed. */
static void null_callback_and_strings(void **)
{
	FallbackWarning::SetMainWindow(main_window, texts());
	FallbackWarning::Warn("test.null", nullptr, nullptr, false, nullptr, nullptr);
	settle();
#ifdef OBS_FRONTEND_RUST
	assert_int_equal(dialogs().size(), 1);
	click(dialogs()[0], "acceptButton");
#endif
	assert_int_equal(logged.size(), 1);
	assert_string_equal(logged[0].text.c_str(),
			    "[fallback] test.null: This may become an error in a future version of OBS.");
}

int main(int argc, char **argv)
{
	qputenv("QT_QPA_PLATFORM", "offscreen");
	QApplication app(argc, argv);

	const struct CMUnitTest tests[] = {
		/* First: before any main window. */
		cmocka_unit_test_setup_teardown(logs_at_once_then_waits_for_the_main_window, setup, teardown),
		cmocka_unit_test_setup_teardown(never_answers_inside_warn, setup, teardown),
		cmocka_unit_test_setup_teardown(no_cancel_for_a_caller_that_cannot_stop, setup, teardown),
		cmocka_unit_test_setup_teardown(one_dialog_at_a_time, setup, teardown),
		cmocka_unit_test_setup_teardown(same_id_shows_once, setup, teardown),
		cmocka_unit_test_setup_teardown(hidden_main_window_accepts, setup, teardown),
		cmocka_unit_test_setup_teardown(waits_while_another_dialog_is_modal, setup, teardown),
		cmocka_unit_test_setup_teardown(shutdown_answers_everyone, setup, teardown),
		cmocka_unit_test_setup_teardown(null_callback_and_strings, setup, teardown),
	};
	if (argc > 1) {
		cmocka_set_test_filter(argv[1]);
	}
	return cmocka_run_group_tests(tests, NULL, NULL);
}
