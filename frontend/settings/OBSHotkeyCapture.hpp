/******************************************************************************
    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#pragma once

class QEvent;
class QKeyEvent;

/* Called first from the event() handler of a Settings > Hotkeys key field
 * (OBSHotkeyEdit). Returns true when the event has been handled and must not
 * reach QLineEdit::event(). */
bool HotkeyCaptureEvent(QEvent *event);

/* Shift+Tab out of a key field (obsproject/obs-studio#12475). Qt delivers the
 * Shift press to the field first, and the field records it as a bare "Shift"
 * binding, before the Tab moves the focus away. The key field keeps one of
 * these: KeyPress() is called from keyPressEvent() before a key is recorded,
 * and Event() from event() before anything else.
 *
 * When a lone modifier key is pressed, the binding from before it is kept.
 * A Tab or Backtab that moves the focus while that modifier is still held
 * puts it back; releasing the modifier, any other key, or leaving the field
 * keeps the new binding. So a modifier pressed and released on its own is
 * still recorded, and a chord with a modifier (Shift+A, Ctrl+Tab) too. */
enum class HotkeyUndoStep { None, MoveFocus, Keep };

bool HotkeyIsModifierKey(const QKeyEvent *event);
HotkeyUndoStep HotkeyUndoStepFor(QEvent *event);

template<typename Key> class HotkeyModifierUndo {
public:
	/* A key press the field is about to record. current/changed are the
	 * field's binding and changed flag before it. */
	void KeyPress(const QKeyEvent *event, const Key &current, bool changed)
	{
		if (!HotkeyIsModifierKey(event)) {
			pending = false;
			return;
		}
		if (pending) {
			return;
		}
		pending = true;
		before = current;
		beforeChanged = changed;
	}

	/* Returns true when key/changed must be put back before the event goes
	 * on to QLineEdit::event(). */
	bool Event(QEvent *event, Key &key, bool &changed)
	{
		if (!pending) {
			return false;
		}
		switch (HotkeyUndoStepFor(event)) {
		case HotkeyUndoStep::MoveFocus:
			pending = false;
			key = before;
			changed = beforeChanged;
			return true;
		case HotkeyUndoStep::Keep:
			pending = false;
			return false;
		case HotkeyUndoStep::None:
			break;
		}
		return false;
	}

	/* Something other than a key press changed the binding (a mouse
	 * button, the Clear or Revert buttons). */
	void Reset() { pending = false; }

private:
	bool pending = false;
	Key before{};
	bool beforeChanged = false;
};
