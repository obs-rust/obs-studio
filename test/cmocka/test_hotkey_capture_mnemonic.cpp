/* Settings > Hotkeys: a key field that is listening for a hotkey must get
 * every key chord, also one that is a shortcut of the dialog
 * (obsproject/obs-studio#13385). With the GNOME and KDE platform themes the
 * dialog's buttons read "&OK" and "&Cancel", so Alt+O and Alt+C are their
 * mnemonics: pressed in the key field they pressed OK or Cancel instead of
 * being recorded.
 *
 * The field below is built like OBSHotkeyEdit: its event() asks
 * HotkeyCaptureEvent() (frontend/settings/OBSHotkeyCapture.cpp) first and its
 * keyPressEvent() records the key. Keys are sent with QTest, which delivers
 * them like the window system does: ShortcutOverride to the focus widget,
 * then the shortcut map, then KeyPress. Qt's offscreen platform, no display. */

#include <QAction>
#include <QApplication>
#include <QDialog>
#include <QDialogButtonBox>
#include <QKeyEvent>
#include <QLineEdit>
#include <QPushButton>
#include <QTest>
#include <QVBoxLayout>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
extern "C" {
#include <cmocka.h>
}

#include "OBSHotkeyCapture.hpp"

class KeyField : public QLineEdit {
public:
	int presses = 0;
	int lastKey = 0;
	Qt::KeyboardModifiers lastModifiers;

protected:
	bool event(QEvent *event) override
	{
		if (HotkeyCaptureEvent(event)) {
			return true;
		}
		return QLineEdit::event(event);
	}

	/* Counts the non-modifier keys it gets. */
	void keyPressEvent(QKeyEvent *event) override
	{
		switch (event->key()) {
		case Qt::Key_Shift:
		case Qt::Key_Control:
		case Qt::Key_Alt:
		case Qt::Key_Meta:
			return;
		}
		presses++;
		lastKey = event->key();
		lastModifiers = event->modifiers();
	}
};

struct Fixture {
	QDialog dialog;
	KeyField *field;
	QLineEdit *other;
	QDialogButtonBox *buttons;
	QAction *closeAction;
	int accepted = 0;
	int rejected = 0;
	int closeTriggered = 0;
};

static int setup(void **state)
{
	auto *f = new Fixture;
	auto *layout = new QVBoxLayout(&f->dialog);
	f->field = new KeyField;
	f->other = new QLineEdit;
	f->buttons = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel | QDialogButtonBox::Apply);
	/* What QGnomeTheme and the KDE platform theme call them. */
	f->buttons->button(QDialogButtonBox::Ok)->setText("&OK");
	f->buttons->button(QDialogButtonBox::Cancel)->setText("&Cancel");
	f->buttons->button(QDialogButtonBox::Apply)->setText("Apply");
	layout->addWidget(f->field);
	layout->addWidget(f->other);
	layout->addWidget(f->buttons);
	QObject::connect(f->buttons, &QDialogButtonBox::accepted, &f->dialog, &QDialog::accept);
	QObject::connect(f->buttons, &QDialogButtonBox::rejected, &f->dialog, &QDialog::reject);
	QObject::connect(&f->dialog, &QDialog::accepted, [f]() { f->accepted++; });
	QObject::connect(&f->dialog, &QDialog::rejected, [f]() { f->rejected++; });

	/* A window shortcut of the dialog itself. */
	f->closeAction = new QAction(&f->dialog);
	f->closeAction->setShortcut(QKeySequence(Qt::CTRL | Qt::Key_W));
	f->dialog.addAction(f->closeAction);
	QObject::connect(f->closeAction, &QAction::triggered, [f]() { f->closeTriggered++; });

	f->dialog.show();
	f->dialog.activateWindow();
	if (!QTest::qWaitForWindowActive(&f->dialog)) {
		delete f;
		return -1;
	}
	*state = f;
	return 0;
}

static int teardown(void **state)
{
	delete static_cast<Fixture *>(*state);
	return 0;
}

/* A mnemonic clicks its button through animateClick(), which fires a moment
 * later: give it time before looking. */
static void press(QWidget *w, Qt::Key key, Qt::KeyboardModifiers modifiers = Qt::NoModifier)
{
	QTest::keyClick(w, key, modifiers);
	QTest::qWait(300);
}

static void focus(QWidget *w)
{
	w->setFocus(Qt::MouseFocusReason);
	QApplication::processEvents();
	assert_true(QApplication::focusWidget() == w);
}

/* Alt+C in the key field: recorded, Cancel not pressed. */
static void alt_c_is_recorded_not_cancel(void **state)
{
	auto *f = static_cast<Fixture *>(*state);
	focus(f->field);

	press(f->field, Qt::Key_C, Qt::AltModifier);

	assert_int_equal(f->rejected, 0);
	assert_true(f->dialog.isVisible());
	assert_int_equal(f->field->presses, 1);
	assert_int_equal(f->field->lastKey, Qt::Key_C);
	assert_true(f->field->lastModifiers == Qt::AltModifier);
}

/* Alt+O in the key field: recorded, OK not pressed. */
static void alt_o_is_recorded_not_ok(void **state)
{
	auto *f = static_cast<Fixture *>(*state);
	focus(f->field);

	press(f->field, Qt::Key_O, Qt::AltModifier);

	assert_int_equal(f->accepted, 0);
	assert_true(f->dialog.isVisible());
	assert_int_equal(f->field->presses, 1);
	assert_int_equal(f->field->lastKey, Qt::Key_O);
	assert_true(f->field->lastModifiers == Qt::AltModifier);
}

/* A window shortcut of the dialog (Ctrl+W) is recorded too. */
static void window_shortcut_is_recorded(void **state)
{
	auto *f = static_cast<Fixture *>(*state);
	focus(f->field);

	press(f->field, Qt::Key_W, Qt::ControlModifier);

	assert_int_equal(f->closeTriggered, 0);
	assert_int_equal(f->field->presses, 1);
	assert_int_equal(f->field->lastKey, Qt::Key_W);
	assert_true(f->field->lastModifiers == Qt::ControlModifier);
}

/* Control: in an ordinary field of the same dialog the mnemonics and the
 * window shortcut still work, so the keys above did reach the shortcut map. */
static void other_field_keeps_mnemonics(void **state)
{
	auto *f = static_cast<Fixture *>(*state);
	focus(f->other);

	press(f->other, Qt::Key_W, Qt::ControlModifier);
	assert_int_equal(f->closeTriggered, 1);

	press(f->other, Qt::Key_C, Qt::AltModifier);
	assert_int_equal(f->rejected, 1);
	assert_false(f->dialog.isVisible());
	assert_int_equal(f->field->presses, 0);
}

/* Control: Tab still moves the focus out of the key field. */
static void tab_leaves_the_field(void **state)
{
	auto *f = static_cast<Fixture *>(*state);
	focus(f->field);

	press(f->field, Qt::Key_Tab);

	assert_true(QApplication::focusWidget() == f->other);
	assert_int_equal(f->field->presses, 0);
	assert_true(f->dialog.isVisible());
}

int main(int argc, char **argv)
{
	qputenv("QT_QPA_PLATFORM", "offscreen");
	QApplication app(argc, argv);

	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(alt_c_is_recorded_not_cancel, setup, teardown),
		cmocka_unit_test_setup_teardown(alt_o_is_recorded_not_ok, setup, teardown),
		cmocka_unit_test_setup_teardown(window_shortcut_is_recorded, setup, teardown),
		cmocka_unit_test_setup_teardown(other_field_keeps_mnemonics, setup, teardown),
		cmocka_unit_test_setup_teardown(tab_leaves_the_field, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
