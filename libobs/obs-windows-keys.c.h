/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

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

#include "libobs/build/libobs-ported-windows-keys.pch.h"

/* Ported to rust/obs-hotkey (windows_keys): the hybrid libobs drops this
 * file. */

/* The Windows key map and key name route of obs-windows.c, moved: the
 * decisions, on plain values. obs-windows.c keeps the platform context, every
 * Windows call, the translations and every dstr call. Declared in
 * obs-windows-keys.h without EXPORT. */

/* get_virtual_key(). */
int obs_windows_keys_virtual_key(obs_key_t key)
{
	switch (key) {
	case OBS_KEY_RETURN:
		return OBS_WINDOWS_KEYS_VK_RETURN;
	case OBS_KEY_ESCAPE:
		return OBS_WINDOWS_KEYS_VK_ESCAPE;
	case OBS_KEY_TAB:
		return OBS_WINDOWS_KEYS_VK_TAB;
	case OBS_KEY_BACKTAB:
		return OBS_WINDOWS_KEYS_VK_OEM_BACKTAB;
	case OBS_KEY_BACKSPACE:
		return OBS_WINDOWS_KEYS_VK_BACK;
	case OBS_KEY_INSERT:
		return OBS_WINDOWS_KEYS_VK_INSERT;
	case OBS_KEY_DELETE:
		return OBS_WINDOWS_KEYS_VK_DELETE;
	case OBS_KEY_PAUSE:
		return OBS_WINDOWS_KEYS_VK_PAUSE;
	case OBS_KEY_PRINT:
		return OBS_WINDOWS_KEYS_VK_SNAPSHOT;
	case OBS_KEY_CLEAR:
		return OBS_WINDOWS_KEYS_VK_CLEAR;
	case OBS_KEY_HOME:
		return OBS_WINDOWS_KEYS_VK_HOME;
	case OBS_KEY_END:
		return OBS_WINDOWS_KEYS_VK_END;
	case OBS_KEY_LEFT:
		return OBS_WINDOWS_KEYS_VK_LEFT;
	case OBS_KEY_UP:
		return OBS_WINDOWS_KEYS_VK_UP;
	case OBS_KEY_RIGHT:
		return OBS_WINDOWS_KEYS_VK_RIGHT;
	case OBS_KEY_DOWN:
		return OBS_WINDOWS_KEYS_VK_DOWN;
	case OBS_KEY_PAGEUP:
		return OBS_WINDOWS_KEYS_VK_PRIOR;
	case OBS_KEY_PAGEDOWN:
		return OBS_WINDOWS_KEYS_VK_NEXT;

	case OBS_KEY_SHIFT:
		return OBS_WINDOWS_KEYS_VK_SHIFT;
	case OBS_KEY_CONTROL:
		return OBS_WINDOWS_KEYS_VK_CONTROL;
	case OBS_KEY_ALT:
		return OBS_WINDOWS_KEYS_VK_MENU;
	case OBS_KEY_CAPSLOCK:
		return OBS_WINDOWS_KEYS_VK_CAPITAL;
	case OBS_KEY_NUMLOCK:
		return OBS_WINDOWS_KEYS_VK_NUMLOCK;
	case OBS_KEY_SCROLLLOCK:
		return OBS_WINDOWS_KEYS_VK_SCROLL;

	case OBS_KEY_F1:
		return OBS_WINDOWS_KEYS_VK_F1;
	case OBS_KEY_F2:
		return OBS_WINDOWS_KEYS_VK_F2;
	case OBS_KEY_F3:
		return OBS_WINDOWS_KEYS_VK_F3;
	case OBS_KEY_F4:
		return OBS_WINDOWS_KEYS_VK_F4;
	case OBS_KEY_F5:
		return OBS_WINDOWS_KEYS_VK_F5;
	case OBS_KEY_F6:
		return OBS_WINDOWS_KEYS_VK_F6;
	case OBS_KEY_F7:
		return OBS_WINDOWS_KEYS_VK_F7;
	case OBS_KEY_F8:
		return OBS_WINDOWS_KEYS_VK_F8;
	case OBS_KEY_F9:
		return OBS_WINDOWS_KEYS_VK_F9;
	case OBS_KEY_F10:
		return OBS_WINDOWS_KEYS_VK_F10;
	case OBS_KEY_F11:
		return OBS_WINDOWS_KEYS_VK_F11;
	case OBS_KEY_F12:
		return OBS_WINDOWS_KEYS_VK_F12;
	case OBS_KEY_F13:
		return OBS_WINDOWS_KEYS_VK_F13;
	case OBS_KEY_F14:
		return OBS_WINDOWS_KEYS_VK_F14;
	case OBS_KEY_F15:
		return OBS_WINDOWS_KEYS_VK_F15;
	case OBS_KEY_F16:
		return OBS_WINDOWS_KEYS_VK_F16;
	case OBS_KEY_F17:
		return OBS_WINDOWS_KEYS_VK_F17;
	case OBS_KEY_F18:
		return OBS_WINDOWS_KEYS_VK_F18;
	case OBS_KEY_F19:
		return OBS_WINDOWS_KEYS_VK_F19;
	case OBS_KEY_F20:
		return OBS_WINDOWS_KEYS_VK_F20;
	case OBS_KEY_F21:
		return OBS_WINDOWS_KEYS_VK_F21;
	case OBS_KEY_F22:
		return OBS_WINDOWS_KEYS_VK_F22;
	case OBS_KEY_F23:
		return OBS_WINDOWS_KEYS_VK_F23;
	case OBS_KEY_F24:
		return OBS_WINDOWS_KEYS_VK_F24;

	case OBS_KEY_SPACE:
		return OBS_WINDOWS_KEYS_VK_SPACE;

	case OBS_KEY_APOSTROPHE:
		return OBS_WINDOWS_KEYS_VK_OEM_7;
	case OBS_KEY_PLUS:
		return OBS_WINDOWS_KEYS_VK_OEM_PLUS;
	case OBS_KEY_COMMA:
		return OBS_WINDOWS_KEYS_VK_OEM_COMMA;
	case OBS_KEY_MINUS:
		return OBS_WINDOWS_KEYS_VK_OEM_MINUS;
	case OBS_KEY_PERIOD:
		return OBS_WINDOWS_KEYS_VK_OEM_PERIOD;
	case OBS_KEY_SLASH:
		return OBS_WINDOWS_KEYS_VK_OEM_2;
	case OBS_KEY_0:
		return '0';
	case OBS_KEY_1:
		return '1';
	case OBS_KEY_2:
		return '2';
	case OBS_KEY_3:
		return '3';
	case OBS_KEY_4:
		return '4';
	case OBS_KEY_5:
		return '5';
	case OBS_KEY_6:
		return '6';
	case OBS_KEY_7:
		return '7';
	case OBS_KEY_8:
		return '8';
	case OBS_KEY_9:
		return '9';
	case OBS_KEY_NUMASTERISK:
		return OBS_WINDOWS_KEYS_VK_MULTIPLY;
	case OBS_KEY_NUMPLUS:
		return OBS_WINDOWS_KEYS_VK_ADD;
	case OBS_KEY_NUMMINUS:
		return OBS_WINDOWS_KEYS_VK_SUBTRACT;
	case OBS_KEY_NUMPERIOD:
		return OBS_WINDOWS_KEYS_VK_DECIMAL;
	case OBS_KEY_NUMSLASH:
		return OBS_WINDOWS_KEYS_VK_DIVIDE;
	case OBS_KEY_NUM0:
		return OBS_WINDOWS_KEYS_VK_NUMPAD0;
	case OBS_KEY_NUM1:
		return OBS_WINDOWS_KEYS_VK_NUMPAD1;
	case OBS_KEY_NUM2:
		return OBS_WINDOWS_KEYS_VK_NUMPAD2;
	case OBS_KEY_NUM3:
		return OBS_WINDOWS_KEYS_VK_NUMPAD3;
	case OBS_KEY_NUM4:
		return OBS_WINDOWS_KEYS_VK_NUMPAD4;
	case OBS_KEY_NUM5:
		return OBS_WINDOWS_KEYS_VK_NUMPAD5;
	case OBS_KEY_NUM6:
		return OBS_WINDOWS_KEYS_VK_NUMPAD6;
	case OBS_KEY_NUM7:
		return OBS_WINDOWS_KEYS_VK_NUMPAD7;
	case OBS_KEY_NUM8:
		return OBS_WINDOWS_KEYS_VK_NUMPAD8;
	case OBS_KEY_NUM9:
		return OBS_WINDOWS_KEYS_VK_NUMPAD9;
	case OBS_KEY_SEMICOLON:
		return OBS_WINDOWS_KEYS_VK_OEM_1;
	case OBS_KEY_A:
		return 'A';
	case OBS_KEY_B:
		return 'B';
	case OBS_KEY_C:
		return 'C';
	case OBS_KEY_D:
		return 'D';
	case OBS_KEY_E:
		return 'E';
	case OBS_KEY_F:
		return 'F';
	case OBS_KEY_G:
		return 'G';
	case OBS_KEY_H:
		return 'H';
	case OBS_KEY_I:
		return 'I';
	case OBS_KEY_J:
		return 'J';
	case OBS_KEY_K:
		return 'K';
	case OBS_KEY_L:
		return 'L';
	case OBS_KEY_M:
		return 'M';
	case OBS_KEY_N:
		return 'N';
	case OBS_KEY_O:
		return 'O';
	case OBS_KEY_P:
		return 'P';
	case OBS_KEY_Q:
		return 'Q';
	case OBS_KEY_R:
		return 'R';
	case OBS_KEY_S:
		return 'S';
	case OBS_KEY_T:
		return 'T';
	case OBS_KEY_U:
		return 'U';
	case OBS_KEY_V:
		return 'V';
	case OBS_KEY_W:
		return 'W';
	case OBS_KEY_X:
		return 'X';
	case OBS_KEY_Y:
		return 'Y';
	case OBS_KEY_Z:
		return 'Z';
	case OBS_KEY_BRACKETLEFT:
		return OBS_WINDOWS_KEYS_VK_OEM_4;
	case OBS_KEY_BACKSLASH:
		return OBS_WINDOWS_KEYS_VK_OEM_5;
	case OBS_KEY_BRACKETRIGHT:
		return OBS_WINDOWS_KEYS_VK_OEM_6;
	case OBS_KEY_ASCIITILDE:
		return OBS_WINDOWS_KEYS_VK_OEM_3;

	case OBS_KEY_HENKAN:
		return OBS_WINDOWS_KEYS_VK_CONVERT;
	case OBS_KEY_MUHENKAN:
		return OBS_WINDOWS_KEYS_VK_NONCONVERT;
	case OBS_KEY_KANJI:
		return OBS_WINDOWS_KEYS_VK_KANJI;
	case OBS_KEY_TOUROKU:
		return OBS_WINDOWS_KEYS_VK_OEM_FJ_TOUROKU;
	case OBS_KEY_MASSYO:
		return OBS_WINDOWS_KEYS_VK_OEM_FJ_MASSHOU;

	case OBS_KEY_HANGUL:
		return OBS_WINDOWS_KEYS_VK_HANGUL;

	case OBS_KEY_BACKSLASH_RT102:
		return OBS_WINDOWS_KEYS_VK_OEM_102;

	case OBS_KEY_MOUSE1:
		return OBS_WINDOWS_KEYS_VK_LBUTTON;
	case OBS_KEY_MOUSE2:
		return OBS_WINDOWS_KEYS_VK_RBUTTON;
	case OBS_KEY_MOUSE3:
		return OBS_WINDOWS_KEYS_VK_MBUTTON;
	case OBS_KEY_MOUSE4:
		return OBS_WINDOWS_KEYS_VK_XBUTTON1;
	case OBS_KEY_MOUSE5:
		return OBS_WINDOWS_KEYS_VK_XBUTTON2;

	case OBS_KEY_VK_CANCEL:
		return OBS_WINDOWS_KEYS_VK_CANCEL;
	case OBS_KEY_0x07:
		return 0x07;
	case OBS_KEY_0x0A:
		return 0x0A;
	case OBS_KEY_0x0B:
		return 0x0B;
	case OBS_KEY_0x0E:
		return 0x0E;
	case OBS_KEY_0x0F:
		return 0x0F;
	case OBS_KEY_0x16:
		return 0x16;
	case OBS_KEY_VK_JUNJA:
		return OBS_WINDOWS_KEYS_VK_JUNJA;
	case OBS_KEY_VK_FINAL:
		return OBS_WINDOWS_KEYS_VK_FINAL;
	case OBS_KEY_0x1A:
		return 0x1A;
	case OBS_KEY_VK_ACCEPT:
		return OBS_WINDOWS_KEYS_VK_ACCEPT;
	case OBS_KEY_VK_MODECHANGE:
		return OBS_WINDOWS_KEYS_VK_MODECHANGE;
	case OBS_KEY_VK_SELECT:
		return OBS_WINDOWS_KEYS_VK_SELECT;
	case OBS_KEY_VK_PRINT:
		return OBS_WINDOWS_KEYS_VK_PRINT;
	case OBS_KEY_VK_EXECUTE:
		return OBS_WINDOWS_KEYS_VK_EXECUTE;
	case OBS_KEY_VK_HELP:
		return OBS_WINDOWS_KEYS_VK_HELP;
	case OBS_KEY_0x30:
		return 0x30;
	case OBS_KEY_0x31:
		return 0x31;
	case OBS_KEY_0x32:
		return 0x32;
	case OBS_KEY_0x33:
		return 0x33;
	case OBS_KEY_0x34:
		return 0x34;
	case OBS_KEY_0x35:
		return 0x35;
	case OBS_KEY_0x36:
		return 0x36;
	case OBS_KEY_0x37:
		return 0x37;
	case OBS_KEY_0x38:
		return 0x38;
	case OBS_KEY_0x39:
		return 0x39;
	case OBS_KEY_0x3A:
		return 0x3A;
	case OBS_KEY_0x3B:
		return 0x3B;
	case OBS_KEY_0x3C:
		return 0x3C;
	case OBS_KEY_0x3D:
		return 0x3D;
	case OBS_KEY_0x3E:
		return 0x3E;
	case OBS_KEY_0x3F:
		return 0x3F;
	case OBS_KEY_0x40:
		return 0x40;
	case OBS_KEY_0x41:
		return 0x41;
	case OBS_KEY_0x42:
		return 0x42;
	case OBS_KEY_0x43:
		return 0x43;
	case OBS_KEY_0x44:
		return 0x44;
	case OBS_KEY_0x45:
		return 0x45;
	case OBS_KEY_0x46:
		return 0x46;
	case OBS_KEY_0x47:
		return 0x47;
	case OBS_KEY_0x48:
		return 0x48;
	case OBS_KEY_0x49:
		return 0x49;
	case OBS_KEY_0x4A:
		return 0x4A;
	case OBS_KEY_0x4B:
		return 0x4B;
	case OBS_KEY_0x4C:
		return 0x4C;
	case OBS_KEY_0x4D:
		return 0x4D;
	case OBS_KEY_0x4E:
		return 0x4E;
	case OBS_KEY_0x4F:
		return 0x4F;
	case OBS_KEY_0x50:
		return 0x50;
	case OBS_KEY_0x51:
		return 0x51;
	case OBS_KEY_0x52:
		return 0x52;
	case OBS_KEY_0x53:
		return 0x53;
	case OBS_KEY_0x54:
		return 0x54;
	case OBS_KEY_0x55:
		return 0x55;
	case OBS_KEY_0x56:
		return 0x56;
	case OBS_KEY_0x57:
		return 0x57;
	case OBS_KEY_0x58:
		return 0x58;
	case OBS_KEY_0x59:
		return 0x59;
	case OBS_KEY_0x5A:
		return 0x5A;
	case OBS_KEY_VK_LWIN:
		return OBS_WINDOWS_KEYS_VK_LWIN;
	case OBS_KEY_VK_RWIN:
		return OBS_WINDOWS_KEYS_VK_RWIN;
	case OBS_KEY_VK_APPS:
		return OBS_WINDOWS_KEYS_VK_APPS;
	case OBS_KEY_0x5E:
		return 0x5E;
	case OBS_KEY_VK_SLEEP:
		return OBS_WINDOWS_KEYS_VK_SLEEP;
	case OBS_KEY_VK_SEPARATOR:
		return OBS_WINDOWS_KEYS_VK_SEPARATOR;
	case OBS_KEY_0x88:
		return 0x88;
	case OBS_KEY_0x89:
		return 0x89;
	case OBS_KEY_0x8A:
		return 0x8A;
	case OBS_KEY_0x8B:
		return 0x8B;
	case OBS_KEY_0x8C:
		return 0x8C;
	case OBS_KEY_0x8D:
		return 0x8D;
	case OBS_KEY_0x8E:
		return 0x8E;
	case OBS_KEY_0x8F:
		return 0x8F;
	case OBS_KEY_VK_OEM_FJ_JISHO:
		return OBS_WINDOWS_KEYS_VK_OEM_FJ_JISHO;
	case OBS_KEY_VK_OEM_FJ_LOYA:
		return OBS_WINDOWS_KEYS_VK_OEM_FJ_LOYA;
	case OBS_KEY_VK_OEM_FJ_ROYA:
		return OBS_WINDOWS_KEYS_VK_OEM_FJ_ROYA;
	case OBS_KEY_0x97:
		return 0x97;
	case OBS_KEY_0x98:
		return 0x98;
	case OBS_KEY_0x99:
		return 0x99;
	case OBS_KEY_0x9A:
		return 0x9A;
	case OBS_KEY_0x9B:
		return 0x9B;
	case OBS_KEY_0x9C:
		return 0x9C;
	case OBS_KEY_0x9D:
		return 0x9D;
	case OBS_KEY_0x9E:
		return 0x9E;
	case OBS_KEY_0x9F:
		return 0x9F;
	case OBS_KEY_VK_LSHIFT:
		return OBS_WINDOWS_KEYS_VK_LSHIFT;
	case OBS_KEY_VK_RSHIFT:
		return OBS_WINDOWS_KEYS_VK_RSHIFT;
	case OBS_KEY_VK_LCONTROL:
		return OBS_WINDOWS_KEYS_VK_LCONTROL;
	case OBS_KEY_VK_RCONTROL:
		return OBS_WINDOWS_KEYS_VK_RCONTROL;
	case OBS_KEY_VK_LMENU:
		return OBS_WINDOWS_KEYS_VK_LMENU;
	case OBS_KEY_VK_RMENU:
		return OBS_WINDOWS_KEYS_VK_RMENU;
	case OBS_KEY_VK_BROWSER_BACK:
		return OBS_WINDOWS_KEYS_VK_BROWSER_BACK;
	case OBS_KEY_VK_BROWSER_FORWARD:
		return OBS_WINDOWS_KEYS_VK_BROWSER_FORWARD;
	case OBS_KEY_VK_BROWSER_REFRESH:
		return OBS_WINDOWS_KEYS_VK_BROWSER_REFRESH;
	case OBS_KEY_VK_BROWSER_STOP:
		return OBS_WINDOWS_KEYS_VK_BROWSER_STOP;
	case OBS_KEY_VK_BROWSER_SEARCH:
		return OBS_WINDOWS_KEYS_VK_BROWSER_SEARCH;
	case OBS_KEY_VK_BROWSER_FAVORITES:
		return OBS_WINDOWS_KEYS_VK_BROWSER_FAVORITES;
	case OBS_KEY_VK_BROWSER_HOME:
		return OBS_WINDOWS_KEYS_VK_BROWSER_HOME;
	case OBS_KEY_VK_VOLUME_MUTE:
		return OBS_WINDOWS_KEYS_VK_VOLUME_MUTE;
	case OBS_KEY_VK_VOLUME_DOWN:
		return OBS_WINDOWS_KEYS_VK_VOLUME_DOWN;
	case OBS_KEY_VK_VOLUME_UP:
		return OBS_WINDOWS_KEYS_VK_VOLUME_UP;
	case OBS_KEY_VK_MEDIA_NEXT_TRACK:
		return OBS_WINDOWS_KEYS_VK_MEDIA_NEXT_TRACK;
	case OBS_KEY_VK_MEDIA_PREV_TRACK:
		return OBS_WINDOWS_KEYS_VK_MEDIA_PREV_TRACK;
	case OBS_KEY_VK_MEDIA_STOP:
		return OBS_WINDOWS_KEYS_VK_MEDIA_STOP;
	case OBS_KEY_VK_MEDIA_PLAY_PAUSE:
		return OBS_WINDOWS_KEYS_VK_MEDIA_PLAY_PAUSE;
	case OBS_KEY_VK_LAUNCH_MAIL:
		return OBS_WINDOWS_KEYS_VK_LAUNCH_MAIL;
	case OBS_KEY_VK_LAUNCH_MEDIA_SELECT:
		return OBS_WINDOWS_KEYS_VK_LAUNCH_MEDIA_SELECT;
	case OBS_KEY_VK_LAUNCH_APP1:
		return OBS_WINDOWS_KEYS_VK_LAUNCH_APP1;
	case OBS_KEY_VK_LAUNCH_APP2:
		return OBS_WINDOWS_KEYS_VK_LAUNCH_APP2;
	case OBS_KEY_0xB8:
		return 0xB8;
	case OBS_KEY_0xB9:
		return 0xB9;
	case OBS_KEY_0xC1:
		return 0xC1;
	case OBS_KEY_0xC2:
		return 0xC2;
	case OBS_KEY_0xC3:
		return 0xC3;
	case OBS_KEY_0xC4:
		return 0xC4;
	case OBS_KEY_0xC5:
		return 0xC5;
	case OBS_KEY_0xC6:
		return 0xC6;
	case OBS_KEY_0xC7:
		return 0xC7;
	case OBS_KEY_0xC8:
		return 0xC8;
	case OBS_KEY_0xC9:
		return 0xC9;
	case OBS_KEY_0xCA:
		return 0xCA;
	case OBS_KEY_0xCB:
		return 0xCB;
	case OBS_KEY_0xCC:
		return 0xCC;
	case OBS_KEY_0xCD:
		return 0xCD;
	case OBS_KEY_0xCE:
		return 0xCE;
	case OBS_KEY_0xCF:
		return 0xCF;
	case OBS_KEY_0xD0:
		return 0xD0;
	case OBS_KEY_0xD1:
		return 0xD1;
	case OBS_KEY_0xD2:
		return 0xD2;
	case OBS_KEY_0xD3:
		return 0xD3;
	case OBS_KEY_0xD4:
		return 0xD4;
	case OBS_KEY_0xD5:
		return 0xD5;
	case OBS_KEY_0xD6:
		return 0xD6;
	case OBS_KEY_0xD7:
		return 0xD7;
	case OBS_KEY_0xD8:
		return 0xD8;
	case OBS_KEY_0xD9:
		return 0xD9;
	case OBS_KEY_0xDA:
		return 0xDA;
	case OBS_KEY_VK_OEM_8:
		return OBS_WINDOWS_KEYS_VK_OEM_8;
	case OBS_KEY_0xE0:
		return 0xE0;
	case OBS_KEY_VK_OEM_AX:
		return OBS_WINDOWS_KEYS_VK_OEM_AX;
	case OBS_KEY_VK_ICO_HELP:
		return OBS_WINDOWS_KEYS_VK_ICO_HELP;
	case OBS_KEY_VK_ICO_00:
		return OBS_WINDOWS_KEYS_VK_ICO_00;
	case OBS_KEY_VK_PROCESSKEY:
		return OBS_WINDOWS_KEYS_VK_PROCESSKEY;
	case OBS_KEY_VK_ICO_CLEAR:
		return OBS_WINDOWS_KEYS_VK_ICO_CLEAR;
	case OBS_KEY_VK_PACKET:
		return OBS_WINDOWS_KEYS_VK_PACKET;
	case OBS_KEY_0xE8:
		return 0xE8;
	case OBS_KEY_VK_OEM_RESET:
		return OBS_WINDOWS_KEYS_VK_OEM_RESET;
	case OBS_KEY_VK_OEM_JUMP:
		return OBS_WINDOWS_KEYS_VK_OEM_JUMP;
	case OBS_KEY_VK_OEM_PA1:
		return OBS_WINDOWS_KEYS_VK_OEM_PA1;
	case OBS_KEY_VK_OEM_PA2:
		return OBS_WINDOWS_KEYS_VK_OEM_PA2;
	case OBS_KEY_VK_OEM_PA3:
		return OBS_WINDOWS_KEYS_VK_OEM_PA3;
	case OBS_KEY_VK_OEM_WSCTRL:
		return OBS_WINDOWS_KEYS_VK_OEM_WSCTRL;
	case OBS_KEY_VK_OEM_CUSEL:
		return OBS_WINDOWS_KEYS_VK_OEM_CUSEL;
	case OBS_KEY_VK_OEM_ATTN:
		return OBS_WINDOWS_KEYS_VK_OEM_ATTN;
	case OBS_KEY_VK_OEM_FINISH:
		return OBS_WINDOWS_KEYS_VK_OEM_FINISH;
	case OBS_KEY_VK_OEM_COPY:
		return OBS_WINDOWS_KEYS_VK_OEM_COPY;
	case OBS_KEY_VK_OEM_AUTO:
		return OBS_WINDOWS_KEYS_VK_OEM_AUTO;
	case OBS_KEY_VK_OEM_ENLW:
		return OBS_WINDOWS_KEYS_VK_OEM_ENLW;
	case OBS_KEY_VK_ATTN:
		return OBS_WINDOWS_KEYS_VK_ATTN;
	case OBS_KEY_VK_CRSEL:
		return OBS_WINDOWS_KEYS_VK_CRSEL;
	case OBS_KEY_VK_EXSEL:
		return OBS_WINDOWS_KEYS_VK_EXSEL;
	case OBS_KEY_VK_EREOF:
		return OBS_WINDOWS_KEYS_VK_EREOF;
	case OBS_KEY_VK_PLAY:
		return OBS_WINDOWS_KEYS_VK_PLAY;
	case OBS_KEY_VK_ZOOM:
		return OBS_WINDOWS_KEYS_VK_ZOOM;
	case OBS_KEY_VK_NONAME:
		return OBS_WINDOWS_KEYS_VK_NONAME;
	case OBS_KEY_VK_PA1:
		return OBS_WINDOWS_KEYS_VK_PA1;
	case OBS_KEY_VK_OEM_CLEAR:
		return OBS_WINDOWS_KEYS_VK_OEM_CLEAR;

	/* TODO: Implement keys for non-US keyboards */
	default:;
	}
	return 0;
}

/* obs_hotkeys_platform_is_pressed()'s Meta rule. */
bool obs_windows_keys_either_win(obs_key_t key)
{
	return key == OBS_KEY_META;
}

/* obs_key_to_str()'s route, up to the scan code. */
enum obs_windows_keys_name obs_windows_keys_name(obs_key_t key, int *number)
{
	if (key == OBS_KEY_NONE) {
		return OBS_WINDOWS_KEYS_NAME_NONE;

	} else if (key >= OBS_KEY_F13 && key <= OBS_KEY_F24) {
		*number = (int)(key - OBS_KEY_F13 + 13);
		return OBS_WINDOWS_KEYS_NAME_F;
	} else if (key >= OBS_KEY_MOUSE1 && key <= OBS_KEY_MOUSE29) {
		*number = (int)(key - OBS_KEY_MOUSE1 + 1);
		return OBS_WINDOWS_KEYS_NAME_MOUSE;
	}
	if (key == OBS_KEY_PAUSE) {
		return OBS_WINDOWS_KEYS_NAME_PAUSE;

	} else if (key == OBS_KEY_META) {
		return OBS_WINDOWS_KEYS_NAME_META;
	}

	return OBS_WINDOWS_KEYS_NAME_SYSTEM;
}

/* obs_key_to_str()'s scan code. */
uint32_t obs_windows_keys_scan_code(int vk, uint32_t mapped)
{
	uint32_t scan_code = mapped << 16;

	switch (vk) {
	case OBS_WINDOWS_KEYS_VK_HOME:
	case OBS_WINDOWS_KEYS_VK_END:
	case OBS_WINDOWS_KEYS_VK_LEFT:
	case OBS_WINDOWS_KEYS_VK_UP:
	case OBS_WINDOWS_KEYS_VK_RIGHT:
	case OBS_WINDOWS_KEYS_VK_DOWN:
	case OBS_WINDOWS_KEYS_VK_PRIOR:
	case OBS_WINDOWS_KEYS_VK_NEXT:
	case OBS_WINDOWS_KEYS_VK_INSERT:
	case OBS_WINDOWS_KEYS_VK_DELETE:
	case OBS_WINDOWS_KEYS_VK_NUMLOCK:
		scan_code |= OBS_WINDOWS_KEYS_EXTENDED;
	}

	return scan_code;
}

/* obs_key_to_str()'s gate before GetKeyNameTextW(). */
bool obs_windows_keys_system_name(obs_key_t key, uint32_t scan_code)
{
	return (key < OBS_KEY_VK_CANCEL || key > OBS_KEY_VK_OEM_CLEAR) && scan_code != 0;
}

/* obs_key_from_virtual_key(). */
obs_key_t obs_windows_keys_from_virtual_key(const int *vk_codes, int code)
{
	for (size_t i = 0; i < OBS_KEY_LAST_VALUE; i++) {
		if (vk_codes[i] == code) {
			return (obs_key_t)i;
		}
	}

	return OBS_KEY_NONE;
}

/* obs_key_to_virtual_key(). */
int obs_windows_keys_to_virtual_key(obs_key_t key, obs_windows_keys_table_fn vk_codes)
{
	if (key == OBS_KEY_META)
		return OBS_WINDOWS_KEYS_VK_LWIN;

	return vk_codes()[(int)key];
}

/* obs_key_combination_to_str()'s order. */
size_t obs_windows_keys_combination(uint32_t modifiers, obs_key_t key, obs_key_t *keys)
{
	size_t num = 0;

	if ((modifiers & INTERACT_CONTROL_KEY) != 0) {
		keys[num++] = OBS_KEY_CONTROL;
	}
	if ((modifiers & INTERACT_COMMAND_KEY) != 0) {
		keys[num++] = OBS_KEY_META;
	}
	if ((modifiers & INTERACT_ALT_KEY) != 0) {
		keys[num++] = OBS_KEY_ALT;
	}
	if ((modifiers & INTERACT_SHIFT_KEY) != 0) {
		keys[num++] = OBS_KEY_SHIFT;
	}
	if (key != OBS_KEY_NONE) {
		keys[num++] = key;
	}

	return num;
}
