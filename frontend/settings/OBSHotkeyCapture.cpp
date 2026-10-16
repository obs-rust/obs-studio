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

#include "OBSHotkeyCapture.hpp"

#include <QEvent>
#include <QKeyEvent>

bool HotkeyCaptureEvent(QEvent *event)
{
	/* Qt offers every key press to the focus widget as a ShortcutOverride
	 * first. Accepting it delivers the key as a plain KeyPress and skips
	 * the shortcut map, so a chord that is also a shortcut of the dialog
	 * (the "&OK" / "&Cancel" mnemonics of the GNOME and KDE themes, a
	 * window action) is recorded instead of triggering it, as
	 * QKeySequenceEdit does. Tab still moves the focus: QWidget handles it
	 * on the KeyPress. */
	if (event->type() == QEvent::ShortcutOverride) {
		event->accept();
		return true;
	}
	return false;
}

bool HotkeyIsModifierKey(const QKeyEvent *event)
{
	switch (event->key()) {
	case Qt::Key_Shift:
	case Qt::Key_Control:
	case Qt::Key_Alt:
	case Qt::Key_Meta:
		return true;
	default:
		return false;
	}
}

HotkeyUndoStep HotkeyUndoStepFor(QEvent *event)
{
	switch (event->type()) {
	case QEvent::KeyPress: {
		/* The keys QWidget::event() moves the focus with. */
		auto *key = static_cast<QKeyEvent *>(event);
		if (key->modifiers() & (Qt::ControlModifier | Qt::AltModifier)) {
			return HotkeyUndoStep::None;
		}
		if (key->key() == Qt::Key_Tab || key->key() == Qt::Key_Backtab) {
			return HotkeyUndoStep::MoveFocus;
		}
		return HotkeyUndoStep::None;
	}
	case QEvent::KeyRelease: {
		auto *key = static_cast<QKeyEvent *>(event);
		if (!key->isAutoRepeat() && HotkeyIsModifierKey(key)) {
			return HotkeyUndoStep::Keep;
		}
		return HotkeyUndoStep::None;
	}
	case QEvent::FocusOut:
		return HotkeyUndoStep::Keep;
	default:
		return HotkeyUndoStep::None;
	}
}
