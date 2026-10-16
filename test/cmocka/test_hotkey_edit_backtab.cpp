/* Settings > Hotkeys: Shift+Tab out of a key field bound "Shift" to that
 * action instead of only moving the focus back (obsproject/obs-studio#12475).
 * The window system delivers the Shift press first; the key field records it
 * as a bare-modifier binding, and only then does the Backtab move the focus.
 * Keyboard and screen-reader users moving backwards through the list
 * silently bound Shift to every action they passed.
 *
 * The field below is built like OBSHotkeyEdit: its event() asks
 * HotkeyCaptureEvent() and HotkeyModifierUndo::Event()
 * (frontend/settings/OBSHotkeyCapture.hpp) first, and its keyPressEvent()
 * calls HotkeyModifierUndo::KeyPress() and records the key like
 * OBSHotkeyEdit::HandleNewKey() (a lone modifier is recorded with the
 * modifier itself, nothing else; an empty or unchanged chord is ignored).
 * Key events are sent as the window system sends them, with the modifiers
 * held before the event; QKeyEvent::modifiers() then adds a modifier key
 * that is being pressed and drops one that is being released. Qt's offscreen
 * platform, no display. */

#include <QApplication>
#include <QKeyEvent>
#include <QLineEdit>
#include <QTest>
#include <QVBoxLayout>
#include <QWidget>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
extern "C" {
#include <cmocka.h>
}

#include "OBSHotkeyCapture.hpp"

struct Combo {
	int key = 0;
	Qt::KeyboardModifiers modifiers;

	bool empty() const { return key == 0 && modifiers == Qt::NoModifier; }
	bool operator==(const Combo &o) const { return key == o.key && modifiers == o.modifiers; }
	bool operator!=(const Combo &o) const { return !(*this == o); }
};

class KeyField : public QLineEdit {
public:
	Combo key;
	bool changed = false;
	int keyChanged = 0;

protected:
	bool event(QEvent *event) override
	{
		if (HotkeyCaptureEvent(event)) {
			return true;
		}
		Combo previous = key;
		bool previousChanged = changed;
		if (undo.Event(event, previous, previousChanged)) {
			key = previous;
			changed = previousChanged;
			keyChanged++;
		}
		return QLineEdit::event(event);
	}

	void keyPressEvent(QKeyEvent *event) override
	{
		if (event->isAutoRepeat()) {
			return;
		}

		Combo combo;
		switch (event->key()) {
		case Qt::Key_Shift:
		case Qt::Key_Control:
		case Qt::Key_Alt:
		case Qt::Key_Meta:
			break;
		default:
			combo.key = event->key();
		}
		combo.modifiers = event->modifiers();

		undo.KeyPress(event, key, changed);
		if (combo == key || combo.empty()) {
			return;
		}
		key = combo;
		changed = true;
		keyChanged++;
	}

private:
	HotkeyModifierUndo<Combo> undo;
};

struct Fixture {
	QWidget window;
	QLineEdit *before;
	KeyField *field;
	QLineEdit *after;
};

static int setup(void **state)
{
	auto *f = new Fixture;
	auto *layout = new QVBoxLayout(&f->window);
	f->before = new QLineEdit;
	f->field = new KeyField;
	f->after = new QLineEdit;
	layout->addWidget(f->before);
	layout->addWidget(f->field);
	layout->addWidget(f->after);

	f->window.show();
	f->window.activateWindow();
	if (!QTest::qWaitForWindowActive(&f->window)) {
		delete f;
		return -1;
	}
	f->field->setFocus(Qt::TabFocusReason);
	QApplication::processEvents();
	if (QApplication::focusWidget() != f->field) {
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

static void send(QEvent::Type type, int key, Qt::KeyboardModifiers modifiers)
{
	QWidget *target = QApplication::focusWidget();
	assert_non_null(target);
	QKeyEvent event(type, key, modifiers);
	QApplication::sendEvent(target, &event);
	QApplication::processEvents();
}

/* held: the modifiers held before the key goes down or up. */
static void press(int key, Qt::KeyboardModifiers held)
{
	send(QEvent::KeyPress, key, held);
}

static void release(int key, Qt::KeyboardModifiers held)
{
	send(QEvent::KeyRelease, key, held);
}

/* Shift+Tab as the platform sends it. tabKey is Key_Backtab (X11, Windows)
 * or Key_Tab with the Shift modifier; Qt moves the focus back for both. */
static void shift_tab(int tabKey)
{
	press(Qt::Key_Shift, Qt::NoModifier);
	press(tabKey, Qt::ShiftModifier);
	release(tabKey, Qt::ShiftModifier);
	release(Qt::Key_Shift, Qt::ShiftModifier);
}

/* The upstream report: Shift+Tab from an empty key field. The focus goes
 * back, and the field stays empty and unchanged. */
static void backtab_leaves_empty_field_empty(void **state)
{
	auto *f = static_cast<Fixture *>(*state);

	shift_tab(Qt::Key_Backtab);

	assert_true(QApplication::focusWidget() == f->before);
	assert_true(f->field->key.empty());
	assert_false(f->field->changed);
}

/* The same with Shift + Key_Tab. */
static void shift_key_tab_leaves_empty_field_empty(void **state)
{
	auto *f = static_cast<Fixture *>(*state);

	shift_tab(Qt::Key_Tab);

	assert_true(QApplication::focusWidget() == f->before);
	assert_true(f->field->key.empty());
	assert_false(f->field->changed);
}

/* A field that already has a binding keeps it. */
static void backtab_keeps_existing_binding(void **state)
{
	auto *f = static_cast<Fixture *>(*state);
	f->field->key = {Qt::Key_X, Qt::ControlModifier};

	shift_tab(Qt::Key_Backtab);

	assert_true(QApplication::focusWidget() == f->before);
	assert_true(f->field->key == (Combo{Qt::Key_X, Qt::ControlModifier}));
	assert_false(f->field->changed);
}

/* A binding made in this field before the Shift+Tab is kept too, with its
 * changed flag. */
static void backtab_keeps_binding_just_made(void **state)
{
	auto *f = static_cast<Fixture *>(*state);

	press(Qt::Key_F5, Qt::NoModifier);
	release(Qt::Key_F5, Qt::NoModifier);
	assert_true(f->field->key == (Combo{Qt::Key_F5, Qt::NoModifier}));

	shift_tab(Qt::Key_Backtab);

	assert_true(QApplication::focusWidget() == f->before);
	assert_true(f->field->key == (Combo{Qt::Key_F5, Qt::NoModifier}));
	assert_true(f->field->changed);
}

/* Control: a lone Shift, pressed and released, is still a binding, and a
 * Tab afterwards leaves it alone. */
static void lone_shift_still_binds(void **state)
{
	auto *f = static_cast<Fixture *>(*state);

	press(Qt::Key_Shift, Qt::NoModifier);
	release(Qt::Key_Shift, Qt::ShiftModifier);
	assert_true(f->field->key == (Combo{0, Qt::ShiftModifier}));

	press(Qt::Key_Tab, Qt::NoModifier);
	release(Qt::Key_Tab, Qt::NoModifier);

	assert_true(QApplication::focusWidget() == f->after);
	assert_true(f->field->key == (Combo{0, Qt::ShiftModifier}));
	assert_true(f->field->changed);
}

/* Control: a lone Shift, released, then Shift+Tab: the Shift binding was
 * made on purpose before the second Shift press, so it stays. */
static void lone_shift_then_backtab_keeps_shift(void **state)
{
	auto *f = static_cast<Fixture *>(*state);

	press(Qt::Key_Shift, Qt::NoModifier);
	release(Qt::Key_Shift, Qt::ShiftModifier);

	shift_tab(Qt::Key_Backtab);

	assert_true(QApplication::focusWidget() == f->before);
	assert_true(f->field->key == (Combo{0, Qt::ShiftModifier}));
	assert_true(f->field->changed);
}

/* Control: Shift+A is recorded as Shift+A. */
static void shift_letter_binds(void **state)
{
	auto *f = static_cast<Fixture *>(*state);

	press(Qt::Key_Shift, Qt::NoModifier);
	press(Qt::Key_A, Qt::ShiftModifier);
	release(Qt::Key_A, Qt::ShiftModifier);
	release(Qt::Key_Shift, Qt::ShiftModifier);

	assert_true(QApplication::focusWidget() == f->field);
	assert_true(f->field->key == (Combo{Qt::Key_A, Qt::ShiftModifier}));
	assert_true(f->field->changed);
}

/* Control: Ctrl+Tab does not move the focus, so it is recorded. */
static void ctrl_tab_binds(void **state)
{
	auto *f = static_cast<Fixture *>(*state);

	press(Qt::Key_Control, Qt::NoModifier);
	press(Qt::Key_Tab, Qt::ControlModifier);
	release(Qt::Key_Tab, Qt::ControlModifier);
	release(Qt::Key_Control, Qt::ControlModifier);

	assert_true(QApplication::focusWidget() == f->field);
	assert_true(f->field->key == (Combo{Qt::Key_Tab, Qt::ControlModifier}));
}

int main(int argc, char **argv)
{
	qputenv("QT_QPA_PLATFORM", "offscreen");
	QApplication app(argc, argv);

	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(backtab_leaves_empty_field_empty, setup, teardown),
		cmocka_unit_test_setup_teardown(shift_key_tab_leaves_empty_field_empty, setup, teardown),
		cmocka_unit_test_setup_teardown(backtab_keeps_existing_binding, setup, teardown),
		cmocka_unit_test_setup_teardown(backtab_keeps_binding_just_made, setup, teardown),
		cmocka_unit_test_setup_teardown(lone_shift_still_binds, setup, teardown),
		cmocka_unit_test_setup_teardown(lone_shift_then_backtab_keeps_shift, setup, teardown),
		cmocka_unit_test_setup_teardown(shift_letter_binds, setup, teardown),
		cmocka_unit_test_setup_teardown(ctrl_tab_binds, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
