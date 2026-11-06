/* The camera-busy warning window (obs-rust/obs-studio#3724). Another program
 * holds a camera OBS uses; the window names it and waits for a click: Close
 * it, Ignore or Always allow. The glue maps the answer to the C ABI of
 * rust/obs-camera-busy, so every way out must report exactly one answer, and
 * the right one; and nothing may answer without a click.
 *
 * The window belongs to the OBS main window: it is that window's child
 * (transient parent), never application-modal, never on top of other apps
 * (no Qt::WindowStaysOnTopHint, no Qt::Tool), and shown in the background it
 * does not take the focus.
 *
 * Builds the real OBSCameraBusy on Qt's offscreen platform, without the
 * frontend (the window takes all texts from its caller), and drives it with
 * Qt Test. */

#include <QApplication>
#include <QLabel>
#include <QPointer>
#include <QPushButton>
#include <QTest>
#include <QWidget>
#include <QWindow>

#include "dialogs/OBSCameraBusy.hpp"

#include <memory>
#include <vector>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
extern "C" {
#include <cmocka.h>
}

using Answer = OBSCameraBusy::Answer;

static OBSCameraBusy::Texts texts(bool can_close, bool known)
{
	OBSCameraBusy::Texts t;
	t.title = "Camera is in use";
	t.message = "OBS cannot use the camera /dev/video0: it is in use by zoom (PID 12).";
	t.closeIt = can_close ? "Close it" : QString();
	t.ignore = "Ignore";
	t.alwaysAllow = known ? "Always allow" : QString();
	return t;
}

struct Shown {
	/* The context of the answered() connection: it goes with this struct,
	 * so a dialog a later teardown closes cannot write into a finished test. */
	QObject context;
	std::unique_ptr<QWidget> main;
	QPointer<OBSCameraBusy> dialog;
	std::vector<Answer> answers;
};

static void show(Shown &s, bool can_close, bool known, bool activate)
{
	s.main = std::make_unique<QWidget>();
	s.main->setWindowTitle("OBS");
	s.main->show();
	s.dialog = new OBSCameraBusy(s.main.get(), texts(can_close, known), activate);
	QObject::connect(s.dialog.data(), &OBSCameraBusy::answered, &s.context,
			 [&s](Answer a) { s.answers.push_back(a); });
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

static void check_one_answer(Shown &s, Answer expected)
{
	assert_int_equal(s.answers.size(), 1);
	assert_int_equal((int)s.answers[0], (int)expected);
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

static void shows_the_callers_texts_and_three_buttons(void **)
{
	Shown s;
	show(s, true, true, true);

	assert_true(s.dialog->windowTitle() == "Camera is in use");
	auto *message = s.dialog->findChild<QLabel *>("message");
	assert_non_null(message);
	assert_true(message->text() == "OBS cannot use the camera /dev/video0: it is in use by zoom (PID 12).");
	auto *close_it = s.dialog->findChild<QPushButton *>("closeItButton");
	auto *ignore = s.dialog->findChild<QPushButton *>("ignoreButton");
	auto *allow = s.dialog->findChild<QPushButton *>("alwaysAllowButton");
	assert_non_null(close_it);
	assert_non_null(ignore);
	assert_non_null(allow);
	assert_true(close_it->text() == "Close it");
	assert_true(ignore->text() == "Ignore");
	assert_true(allow->text() == "Always allow");
	assert_true(close_it->isVisible() && ignore->isVisible() && allow->isVisible());
	assert_true(s.answers.empty());
}

static void no_close_it_or_allow_without_their_texts(void **)
{
	Shown s;
	show(s, false, false, true);

	assert_null(s.dialog->findChild<QPushButton *>("closeItButton"));
	assert_null(s.dialog->findChild<QPushButton *>("alwaysAllowButton"));
	assert_non_null(s.dialog->findChild<QPushButton *>("ignoreButton"));
}

/* The user's requirement: a child of the main window, which minimizes and
 * comes back with it, never floating over other apps. */
static void belongs_to_the_main_window(void **)
{
	Shown s;
	show(s, true, true, true);

	assert_true(s.dialog->parentWidget() == s.main.get());
	assert_true(s.dialog->isWindow());
	assert_true(s.dialog->windowHandle() != nullptr);
	assert_true(s.dialog->windowHandle()->transientParent() == s.main->windowHandle());
	Qt::WindowFlags flags = s.dialog->windowFlags();
	assert_int_equal((int)(flags & Qt::WindowType_Mask), (int)Qt::Dialog);
	assert_false(flags.testFlag(Qt::WindowStaysOnTopHint));
	assert_false(flags.testFlag(Qt::X11BypassWindowManagerHint));
	/* Not modal at all: OBS and every other app keep working. */
	assert_false(s.dialog->isModal());
	assert_int_equal((int)s.dialog->windowModality(), (int)Qt::NonModal);
	assert_null(QApplication::activeModalWidget());
}

static void in_the_background_it_does_not_take_the_focus(void **)
{
	Shown s;
	show(s, true, true, false);
	assert_true(s.dialog->testAttribute(Qt::WA_ShowWithoutActivating));

	Shown f;
	show(f, true, true, true);
	assert_false(f.dialog->testAttribute(Qt::WA_ShowWithoutActivating));
}

/* No countdown, no default button: time and Enter answer nothing. */
static void waits_for_a_click(void **)
{
	Shown s;
	show(s, true, true, true);
	QTest::keyClick(s.dialog.data(), Qt::Key_Return);
	settle(300);
	assert_true(s.answers.empty());
	assert_non_null(s.dialog.data());
	assert_true(s.dialog->isVisible());
}

static void close_it_answers_once(void **)
{
	Shown s;
	show(s, true, true, true);
	click(s, "closeItButton");
	settle(50);
	check_one_answer(s, Answer::CloseIt);
}

static void ignore_answers_once(void **)
{
	Shown s;
	show(s, true, true, true);
	click(s, "ignoreButton");
	settle(50);
	check_one_answer(s, Answer::Ignore);
}

static void always_allow_answers_once(void **)
{
	Shown s;
	show(s, true, true, true);
	click(s, "alwaysAllowButton");
	settle(50);
	check_one_answer(s, Answer::AlwaysAllow);
}

static void escape_is_ignore(void **)
{
	Shown s;
	show(s, true, true, true);
	QTest::keyClick(s.dialog.data(), Qt::Key_Escape);
	settle(50);
	check_one_answer(s, Answer::Ignore);
}

static void window_close_is_ignore(void **)
{
	Shown s;
	show(s, true, true, true);
	s.dialog->close();
	settle(50);
	check_one_answer(s, Answer::Ignore);
}

int main(int argc, char **argv)
{
	qputenv("QT_QPA_PLATFORM", "offscreen");
	QApplication app(argc, argv);

	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(shows_the_callers_texts_and_three_buttons, teardown),
		cmocka_unit_test_teardown(no_close_it_or_allow_without_their_texts, teardown),
		cmocka_unit_test_teardown(belongs_to_the_main_window, teardown),
		cmocka_unit_test_teardown(in_the_background_it_does_not_take_the_focus, teardown),
		cmocka_unit_test_teardown(waits_for_a_click, teardown),
		cmocka_unit_test_teardown(close_it_answers_once, teardown),
		cmocka_unit_test_teardown(ignore_answers_once, teardown),
		cmocka_unit_test_teardown(always_allow_answers_once, teardown),
		cmocka_unit_test_teardown(escape_is_ignore, teardown),
		cmocka_unit_test_teardown(window_close_is_ignore, teardown),
	};
	if (argc > 1) {
		cmocka_set_test_filter(argv[1]);
	}
	return cmocka_run_group_tests(tests, NULL, NULL);
}
