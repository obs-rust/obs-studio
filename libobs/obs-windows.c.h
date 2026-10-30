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

#include "libobs/build/libobs-windows.pch.h"

static uint32_t win_ver = 0;
static uint32_t win_build = 0;

const char *get_module_extension(void)
{
	return ".dll";
}

/* on windows, points to [base directory]/data/libobs */
char *find_libobs_data_file(const char *file)
{
	struct dstr path;
	dstr_init(&path);

	if (check_path(file, "../../data/libobs/", &path))
		return path.array;

	dstr_free(&path);
	return NULL;
}

static void log_processor_info(void)
{
	HKEY key;
	wchar_t data[1024];
	char *str = NULL;
	DWORD size, speed;
	LSTATUS status;

	memset(data, 0, sizeof(data));

	status = RegOpenKeyW(HKEY_LOCAL_MACHINE, L"HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0", &key);
	if (status != ERROR_SUCCESS)
		return;

	size = sizeof(data);
	status = RegQueryValueExW(key, L"ProcessorNameString", NULL, NULL, (LPBYTE)data, &size);
	if (status == ERROR_SUCCESS) {
		os_wcs_to_utf8_ptr(data, 0, &str);
		blog(LOG_INFO, "CPU Name: %s", str);
		bfree(str);
	}

	size = sizeof(speed);
	status = RegQueryValueExW(key, L"~MHz", NULL, NULL, (LPBYTE)&speed, &size);
	if (status == ERROR_SUCCESS)
		blog(LOG_INFO, "CPU Speed: %ldMHz", speed);

	RegCloseKey(key);
}

static void log_processor_cores(void)
{
	blog(LOG_INFO, "Physical Cores: %d, Logical Cores: %d", os_get_physical_cores(), os_get_logical_cores());
}

static void log_emulation_status(void)
{
	if (os_get_emulation_status()) {
		blog(LOG_WARNING, "Windows ARM64: Running with x64 emulation");
	}
}

static void log_available_memory(void)
{
	MEMORYSTATUSEX ms;
	ms.dwLength = sizeof(ms);

	GlobalMemoryStatusEx(&ms);

#ifdef _WIN64
	const char *note = "";
#else
	const char *note = " (NOTE: 32bit programs cannot use more than 3gb)";
#endif

	blog(LOG_INFO, "Physical Memory: %luMB Total, %luMB Free%s", (DWORD)(ms.ullTotalPhys / 1048576),
	     (DWORD)(ms.ullAvailPhys / 1048576), note);
}

static void log_lenovo_vantage(void)
{
	SC_HANDLE manager = OpenSCManager(NULL, NULL, SC_MANAGER_CONNECT);

	if (!manager)
		return;

	SC_HANDLE service = OpenService(manager, L"FBNetFilter", SERVICE_QUERY_STATUS);

	if (service) {
		blog(LOG_WARNING, "Lenovo Vantage / Legion Edge is installed. The \"Network Boost\" "
				  "feature must be disabled when streaming with OBS.");
		CloseServiceHandle(service);
	}

	CloseServiceHandle(manager);
}

static void log_conflicting_software(void)
{
	log_lenovo_vantage();
}

extern const char *get_win_release_id(void);

static void log_windows_version(void)
{
	struct win_version_info ver;
	get_win_ver(&ver);

	const char *release_id = get_win_release_id();

	bool b64 = is_64_bit_windows();
	const char *windows_bitness = b64 ? "64" : "32";

	bool arm64 = is_arm64_windows();
	const char *arm64_windows = arm64 ? "ARM " : "";

	blog(LOG_INFO, "Windows Version: %d.%d Build %d (release: %s; revision: %d; %s%s-bit)", ver.major, ver.minor,
	     ver.build, release_id, ver.revis, arm64_windows, windows_bitness);
}

static void log_admin_status(void)
{
	SID_IDENTIFIER_AUTHORITY auth = SECURITY_NT_AUTHORITY;
	PSID admin_group;
	BOOL success;

	success = AllocateAndInitializeSid(&auth, 2, SECURITY_BUILTIN_DOMAIN_RID, DOMAIN_ALIAS_RID_ADMINS, 0, 0, 0, 0,
					   0, 0, &admin_group);
	if (success) {
		if (!CheckTokenMembership(NULL, admin_group, &success))
			success = false;
		FreeSid(admin_group);
	}

	blog(LOG_INFO, "Running as administrator: %s", success ? "true" : "false");
}

#define WIN10_GAME_BAR_REG_KEY L"Software\\Microsoft\\Windows\\CurrentVersion\\GameDVR"
#define WIN10_GAME_DVR_POLICY_REG_KEY L"SOFTWARE\\Policies\\Microsoft\\Windows\\GameDVR"
#define WIN10_GAME_DVR_REG_KEY L"System\\GameConfigStore"
#define WIN10_GAME_MODE_REG_KEY L"Software\\Microsoft\\GameBar"

static void log_gaming_features(void)
{
	if (win_ver < 0xA00)
		return;

	struct reg_dword game_bar_enabled;
	struct reg_dword game_dvr_allowed;
	struct reg_dword game_dvr_enabled;
	struct reg_dword game_dvr_bg_recording;
	struct reg_dword game_mode_enabled;

	get_reg_dword(HKEY_CURRENT_USER, WIN10_GAME_BAR_REG_KEY, L"AppCaptureEnabled", &game_bar_enabled);
	get_reg_dword(HKEY_CURRENT_USER, WIN10_GAME_DVR_POLICY_REG_KEY, L"AllowGameDVR", &game_dvr_allowed);
	get_reg_dword(HKEY_CURRENT_USER, WIN10_GAME_DVR_REG_KEY, L"GameDVR_Enabled", &game_dvr_enabled);
	get_reg_dword(HKEY_CURRENT_USER, WIN10_GAME_BAR_REG_KEY, L"HistoricalCaptureEnabled", &game_dvr_bg_recording);
	get_reg_dword(HKEY_CURRENT_USER, WIN10_GAME_MODE_REG_KEY, L"AutoGameModeEnabled", &game_mode_enabled);

	if (game_mode_enabled.status != ERROR_SUCCESS) {
		get_reg_dword(HKEY_CURRENT_USER, WIN10_GAME_MODE_REG_KEY, L"AllowAutoGameMode", &game_mode_enabled);
	}

	blog(LOG_INFO, "Windows 10/11 Gaming Features:");
	if (game_bar_enabled.status == ERROR_SUCCESS) {
		blog(LOG_INFO, "\tGame Bar: %s", (bool)game_bar_enabled.return_value ? "On" : "Off");
	}

	if (game_dvr_allowed.status == ERROR_SUCCESS) {
		blog(LOG_INFO, "\tGame DVR Allowed: %s", (bool)game_dvr_allowed.return_value ? "Yes" : "No");
	}

	if (game_dvr_enabled.status == ERROR_SUCCESS) {
		blog(LOG_INFO, "\tGame DVR: %s", (bool)game_dvr_enabled.return_value ? "On" : "Off");
	}

	if (game_dvr_bg_recording.status == ERROR_SUCCESS) {
		blog(LOG_INFO, "\tGame DVR Background Recording: %s",
		     (bool)game_dvr_bg_recording.return_value ? "On" : "Off");
	}

	if (game_mode_enabled.status == ERROR_SUCCESS) {
		blog(LOG_INFO, "\tGame Mode: %s", (bool)game_mode_enabled.return_value ? "On" : "Off");
	} else if (win_build >= 19042) {
		// On by default in newer Windows 10 builds (no registry key set)
		blog(LOG_INFO, "\tGame Mode: Probably On (no reg key set)");
	}
}

static const char *get_str_for_state(int state)
{
	switch (state) {
	case WSC_SECURITY_PRODUCT_STATE_ON:
		return "enabled";
	case WSC_SECURITY_PRODUCT_STATE_OFF:
		return "disabled";
	case WSC_SECURITY_PRODUCT_STATE_SNOOZED:
		return "temporarily disabled";
	case WSC_SECURITY_PRODUCT_STATE_EXPIRED:
		return "expired";
	default:
		return "unknown";
	}
}

static const char *get_str_for_type(int type)
{
	switch (type) {
	case WSC_SECURITY_PROVIDER_ANTIVIRUS:
		return "AV";
	case WSC_SECURITY_PROVIDER_FIREWALL:
		return "FW";
	case WSC_SECURITY_PROVIDER_ANTISPYWARE:
		return "ASW";
	default:
		return "unknown";
	}
}

static void log_security_products_by_type(IWSCProductList *prod_list, int type)
{
	HRESULT hr;
	LONG count = 0;
	IWscProduct *prod;
	BSTR name;
	WSC_SECURITY_PRODUCT_STATE prod_state;

	hr = prod_list->lpVtbl->Initialize(prod_list, type);

	if (FAILED(hr)) {
		prod_list->lpVtbl->Release(prod_list);
		return;
	}

	hr = prod_list->lpVtbl->get_Count(prod_list, &count);
	if (FAILED(hr)) {
		prod_list->lpVtbl->Release(prod_list);
		return;
	}

	for (int i = 0; i < count; i++) {
		hr = prod_list->lpVtbl->get_Item(prod_list, i, &prod);
		if (FAILED(hr))
			continue;

		hr = prod->lpVtbl->get_ProductName(prod, &name);
		if (FAILED(hr)) {
			prod->lpVtbl->Release(prod);
			continue;
		}

		hr = prod->lpVtbl->get_ProductState(prod, &prod_state);
		if (FAILED(hr)) {
			SysFreeString(name);
			prod->lpVtbl->Release(prod);
			continue;
		}

		char *product_name;
		os_wcs_to_utf8_ptr(name, 0, &product_name);

		blog(LOG_INFO, "\t%s: %s (%s)", product_name, get_str_for_state(prod_state), get_str_for_type(type));

		bfree(product_name);

		SysFreeString(name);
		prod->lpVtbl->Release(prod);
	}

	prod_list->lpVtbl->Release(prod_list);
}

static void log_security_products(void)
{
	IWSCProductList *prod_list = NULL;
	HMODULE h_wsc;
	HRESULT hr;

	/* We load the DLL rather than import wcsapi.lib because the clsid /
	 * iid only exists on Windows 8 or higher. */

	h_wsc = LoadLibraryW(L"wscapi.dll");
	if (!h_wsc)
		return;

	const CLSID *prod_list_clsid = (const CLSID *)GetProcAddress(h_wsc, "CLSID_WSCProductList");
	const IID *prod_list_iid = (const IID *)GetProcAddress(h_wsc, "IID_IWSCProductList");

	if (prod_list_clsid && prod_list_iid) {
		blog(LOG_INFO, "Sec. Software Status:");

		hr = CoCreateInstance(prod_list_clsid, NULL, CLSCTX_INPROC_SERVER, prod_list_iid, &prod_list);
		if (!FAILED(hr)) {
			log_security_products_by_type(prod_list, WSC_SECURITY_PROVIDER_ANTIVIRUS);
		}

		hr = CoCreateInstance(prod_list_clsid, NULL, CLSCTX_INPROC_SERVER, prod_list_iid, &prod_list);
		if (!FAILED(hr)) {
			log_security_products_by_type(prod_list, WSC_SECURITY_PROVIDER_FIREWALL);
		}

		hr = CoCreateInstance(prod_list_clsid, NULL, CLSCTX_INPROC_SERVER, prod_list_iid, &prod_list);
		if (!FAILED(hr)) {
			log_security_products_by_type(prod_list, WSC_SECURITY_PROVIDER_ANTISPYWARE);
		}
	}

	FreeLibrary(h_wsc);
}

void log_system_info(void)
{
	struct win_version_info ver;
	get_win_ver(&ver);

	win_ver = (ver.major << 8) | ver.minor;
	win_build = ver.build;

	log_processor_info();
	log_processor_cores();
	log_available_memory();
	log_windows_version();
	log_emulation_status();
	log_admin_status();
	log_gaming_features();
	log_security_products();
	log_conflicting_software();
}

struct obs_hotkeys_platform {
	int vk_codes[OBS_KEY_LAST_VALUE];
};

/* The key map, obs-windows-keys.c, has no windows.h: check what it assumes. */
_Static_assert(sizeof(UINT) == sizeof(uint32_t), "a scan code is a UINT");
_Static_assert(OBS_WINDOWS_KEYS_VK_ACCEPT == VK_ACCEPT, "VK_ACCEPT");
_Static_assert(OBS_WINDOWS_KEYS_VK_ADD == VK_ADD, "VK_ADD");
_Static_assert(OBS_WINDOWS_KEYS_VK_APPS == VK_APPS, "VK_APPS");
_Static_assert(OBS_WINDOWS_KEYS_VK_ATTN == VK_ATTN, "VK_ATTN");
_Static_assert(OBS_WINDOWS_KEYS_VK_BACK == VK_BACK, "VK_BACK");
_Static_assert(OBS_WINDOWS_KEYS_VK_BROWSER_BACK == VK_BROWSER_BACK, "VK_BROWSER_BACK");
_Static_assert(OBS_WINDOWS_KEYS_VK_BROWSER_FAVORITES == VK_BROWSER_FAVORITES, "VK_BROWSER_FAVORITES");
_Static_assert(OBS_WINDOWS_KEYS_VK_BROWSER_FORWARD == VK_BROWSER_FORWARD, "VK_BROWSER_FORWARD");
_Static_assert(OBS_WINDOWS_KEYS_VK_BROWSER_HOME == VK_BROWSER_HOME, "VK_BROWSER_HOME");
_Static_assert(OBS_WINDOWS_KEYS_VK_BROWSER_REFRESH == VK_BROWSER_REFRESH, "VK_BROWSER_REFRESH");
_Static_assert(OBS_WINDOWS_KEYS_VK_BROWSER_SEARCH == VK_BROWSER_SEARCH, "VK_BROWSER_SEARCH");
_Static_assert(OBS_WINDOWS_KEYS_VK_BROWSER_STOP == VK_BROWSER_STOP, "VK_BROWSER_STOP");
_Static_assert(OBS_WINDOWS_KEYS_VK_CANCEL == VK_CANCEL, "VK_CANCEL");
_Static_assert(OBS_WINDOWS_KEYS_VK_CAPITAL == VK_CAPITAL, "VK_CAPITAL");
_Static_assert(OBS_WINDOWS_KEYS_VK_CLEAR == VK_CLEAR, "VK_CLEAR");
_Static_assert(OBS_WINDOWS_KEYS_VK_CONTROL == VK_CONTROL, "VK_CONTROL");
_Static_assert(OBS_WINDOWS_KEYS_VK_CONVERT == VK_CONVERT, "VK_CONVERT");
_Static_assert(OBS_WINDOWS_KEYS_VK_CRSEL == VK_CRSEL, "VK_CRSEL");
_Static_assert(OBS_WINDOWS_KEYS_VK_DECIMAL == VK_DECIMAL, "VK_DECIMAL");
_Static_assert(OBS_WINDOWS_KEYS_VK_DELETE == VK_DELETE, "VK_DELETE");
_Static_assert(OBS_WINDOWS_KEYS_VK_DIVIDE == VK_DIVIDE, "VK_DIVIDE");
_Static_assert(OBS_WINDOWS_KEYS_VK_DOWN == VK_DOWN, "VK_DOWN");
_Static_assert(OBS_WINDOWS_KEYS_VK_END == VK_END, "VK_END");
_Static_assert(OBS_WINDOWS_KEYS_VK_EREOF == VK_EREOF, "VK_EREOF");
_Static_assert(OBS_WINDOWS_KEYS_VK_ESCAPE == VK_ESCAPE, "VK_ESCAPE");
_Static_assert(OBS_WINDOWS_KEYS_VK_EXECUTE == VK_EXECUTE, "VK_EXECUTE");
_Static_assert(OBS_WINDOWS_KEYS_VK_EXSEL == VK_EXSEL, "VK_EXSEL");
_Static_assert(OBS_WINDOWS_KEYS_VK_F1 == VK_F1, "VK_F1");
_Static_assert(OBS_WINDOWS_KEYS_VK_F10 == VK_F10, "VK_F10");
_Static_assert(OBS_WINDOWS_KEYS_VK_F11 == VK_F11, "VK_F11");
_Static_assert(OBS_WINDOWS_KEYS_VK_F12 == VK_F12, "VK_F12");
_Static_assert(OBS_WINDOWS_KEYS_VK_F13 == VK_F13, "VK_F13");
_Static_assert(OBS_WINDOWS_KEYS_VK_F14 == VK_F14, "VK_F14");
_Static_assert(OBS_WINDOWS_KEYS_VK_F15 == VK_F15, "VK_F15");
_Static_assert(OBS_WINDOWS_KEYS_VK_F16 == VK_F16, "VK_F16");
_Static_assert(OBS_WINDOWS_KEYS_VK_F17 == VK_F17, "VK_F17");
_Static_assert(OBS_WINDOWS_KEYS_VK_F18 == VK_F18, "VK_F18");
_Static_assert(OBS_WINDOWS_KEYS_VK_F19 == VK_F19, "VK_F19");
_Static_assert(OBS_WINDOWS_KEYS_VK_F2 == VK_F2, "VK_F2");
_Static_assert(OBS_WINDOWS_KEYS_VK_F20 == VK_F20, "VK_F20");
_Static_assert(OBS_WINDOWS_KEYS_VK_F21 == VK_F21, "VK_F21");
_Static_assert(OBS_WINDOWS_KEYS_VK_F22 == VK_F22, "VK_F22");
_Static_assert(OBS_WINDOWS_KEYS_VK_F23 == VK_F23, "VK_F23");
_Static_assert(OBS_WINDOWS_KEYS_VK_F24 == VK_F24, "VK_F24");
_Static_assert(OBS_WINDOWS_KEYS_VK_F3 == VK_F3, "VK_F3");
_Static_assert(OBS_WINDOWS_KEYS_VK_F4 == VK_F4, "VK_F4");
_Static_assert(OBS_WINDOWS_KEYS_VK_F5 == VK_F5, "VK_F5");
_Static_assert(OBS_WINDOWS_KEYS_VK_F6 == VK_F6, "VK_F6");
_Static_assert(OBS_WINDOWS_KEYS_VK_F7 == VK_F7, "VK_F7");
_Static_assert(OBS_WINDOWS_KEYS_VK_F8 == VK_F8, "VK_F8");
_Static_assert(OBS_WINDOWS_KEYS_VK_F9 == VK_F9, "VK_F9");
_Static_assert(OBS_WINDOWS_KEYS_VK_FINAL == VK_FINAL, "VK_FINAL");
_Static_assert(OBS_WINDOWS_KEYS_VK_HANGUL == VK_HANGUL, "VK_HANGUL");
_Static_assert(OBS_WINDOWS_KEYS_VK_HELP == VK_HELP, "VK_HELP");
_Static_assert(OBS_WINDOWS_KEYS_VK_HOME == VK_HOME, "VK_HOME");
_Static_assert(OBS_WINDOWS_KEYS_VK_ICO_00 == VK_ICO_00, "VK_ICO_00");
_Static_assert(OBS_WINDOWS_KEYS_VK_ICO_CLEAR == VK_ICO_CLEAR, "VK_ICO_CLEAR");
_Static_assert(OBS_WINDOWS_KEYS_VK_ICO_HELP == VK_ICO_HELP, "VK_ICO_HELP");
_Static_assert(OBS_WINDOWS_KEYS_VK_INSERT == VK_INSERT, "VK_INSERT");
_Static_assert(OBS_WINDOWS_KEYS_VK_JUNJA == VK_JUNJA, "VK_JUNJA");
_Static_assert(OBS_WINDOWS_KEYS_VK_KANJI == VK_KANJI, "VK_KANJI");
_Static_assert(OBS_WINDOWS_KEYS_VK_LAUNCH_APP1 == VK_LAUNCH_APP1, "VK_LAUNCH_APP1");
_Static_assert(OBS_WINDOWS_KEYS_VK_LAUNCH_APP2 == VK_LAUNCH_APP2, "VK_LAUNCH_APP2");
_Static_assert(OBS_WINDOWS_KEYS_VK_LAUNCH_MAIL == VK_LAUNCH_MAIL, "VK_LAUNCH_MAIL");
_Static_assert(OBS_WINDOWS_KEYS_VK_LAUNCH_MEDIA_SELECT == VK_LAUNCH_MEDIA_SELECT, "VK_LAUNCH_MEDIA_SELECT");
_Static_assert(OBS_WINDOWS_KEYS_VK_LBUTTON == VK_LBUTTON, "VK_LBUTTON");
_Static_assert(OBS_WINDOWS_KEYS_VK_LCONTROL == VK_LCONTROL, "VK_LCONTROL");
_Static_assert(OBS_WINDOWS_KEYS_VK_LEFT == VK_LEFT, "VK_LEFT");
_Static_assert(OBS_WINDOWS_KEYS_VK_LMENU == VK_LMENU, "VK_LMENU");
_Static_assert(OBS_WINDOWS_KEYS_VK_LSHIFT == VK_LSHIFT, "VK_LSHIFT");
_Static_assert(OBS_WINDOWS_KEYS_VK_LWIN == VK_LWIN, "VK_LWIN");
_Static_assert(OBS_WINDOWS_KEYS_VK_MBUTTON == VK_MBUTTON, "VK_MBUTTON");
_Static_assert(OBS_WINDOWS_KEYS_VK_MEDIA_NEXT_TRACK == VK_MEDIA_NEXT_TRACK, "VK_MEDIA_NEXT_TRACK");
_Static_assert(OBS_WINDOWS_KEYS_VK_MEDIA_PLAY_PAUSE == VK_MEDIA_PLAY_PAUSE, "VK_MEDIA_PLAY_PAUSE");
_Static_assert(OBS_WINDOWS_KEYS_VK_MEDIA_PREV_TRACK == VK_MEDIA_PREV_TRACK, "VK_MEDIA_PREV_TRACK");
_Static_assert(OBS_WINDOWS_KEYS_VK_MEDIA_STOP == VK_MEDIA_STOP, "VK_MEDIA_STOP");
_Static_assert(OBS_WINDOWS_KEYS_VK_MENU == VK_MENU, "VK_MENU");
_Static_assert(OBS_WINDOWS_KEYS_VK_MODECHANGE == VK_MODECHANGE, "VK_MODECHANGE");
_Static_assert(OBS_WINDOWS_KEYS_VK_MULTIPLY == VK_MULTIPLY, "VK_MULTIPLY");
_Static_assert(OBS_WINDOWS_KEYS_VK_NEXT == VK_NEXT, "VK_NEXT");
_Static_assert(OBS_WINDOWS_KEYS_VK_NONAME == VK_NONAME, "VK_NONAME");
_Static_assert(OBS_WINDOWS_KEYS_VK_NONCONVERT == VK_NONCONVERT, "VK_NONCONVERT");
_Static_assert(OBS_WINDOWS_KEYS_VK_NUMLOCK == VK_NUMLOCK, "VK_NUMLOCK");
_Static_assert(OBS_WINDOWS_KEYS_VK_NUMPAD0 == VK_NUMPAD0, "VK_NUMPAD0");
_Static_assert(OBS_WINDOWS_KEYS_VK_NUMPAD1 == VK_NUMPAD1, "VK_NUMPAD1");
_Static_assert(OBS_WINDOWS_KEYS_VK_NUMPAD2 == VK_NUMPAD2, "VK_NUMPAD2");
_Static_assert(OBS_WINDOWS_KEYS_VK_NUMPAD3 == VK_NUMPAD3, "VK_NUMPAD3");
_Static_assert(OBS_WINDOWS_KEYS_VK_NUMPAD4 == VK_NUMPAD4, "VK_NUMPAD4");
_Static_assert(OBS_WINDOWS_KEYS_VK_NUMPAD5 == VK_NUMPAD5, "VK_NUMPAD5");
_Static_assert(OBS_WINDOWS_KEYS_VK_NUMPAD6 == VK_NUMPAD6, "VK_NUMPAD6");
_Static_assert(OBS_WINDOWS_KEYS_VK_NUMPAD7 == VK_NUMPAD7, "VK_NUMPAD7");
_Static_assert(OBS_WINDOWS_KEYS_VK_NUMPAD8 == VK_NUMPAD8, "VK_NUMPAD8");
_Static_assert(OBS_WINDOWS_KEYS_VK_NUMPAD9 == VK_NUMPAD9, "VK_NUMPAD9");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_1 == VK_OEM_1, "VK_OEM_1");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_102 == VK_OEM_102, "VK_OEM_102");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_2 == VK_OEM_2, "VK_OEM_2");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_3 == VK_OEM_3, "VK_OEM_3");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_4 == VK_OEM_4, "VK_OEM_4");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_5 == VK_OEM_5, "VK_OEM_5");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_6 == VK_OEM_6, "VK_OEM_6");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_7 == VK_OEM_7, "VK_OEM_7");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_8 == VK_OEM_8, "VK_OEM_8");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_ATTN == VK_OEM_ATTN, "VK_OEM_ATTN");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_AUTO == VK_OEM_AUTO, "VK_OEM_AUTO");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_AX == VK_OEM_AX, "VK_OEM_AX");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_BACKTAB == VK_OEM_BACKTAB, "VK_OEM_BACKTAB");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_CLEAR == VK_OEM_CLEAR, "VK_OEM_CLEAR");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_COMMA == VK_OEM_COMMA, "VK_OEM_COMMA");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_COPY == VK_OEM_COPY, "VK_OEM_COPY");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_CUSEL == VK_OEM_CUSEL, "VK_OEM_CUSEL");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_ENLW == VK_OEM_ENLW, "VK_OEM_ENLW");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_FINISH == VK_OEM_FINISH, "VK_OEM_FINISH");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_FJ_JISHO == VK_OEM_FJ_JISHO, "VK_OEM_FJ_JISHO");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_FJ_LOYA == VK_OEM_FJ_LOYA, "VK_OEM_FJ_LOYA");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_FJ_MASSHOU == VK_OEM_FJ_MASSHOU, "VK_OEM_FJ_MASSHOU");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_FJ_ROYA == VK_OEM_FJ_ROYA, "VK_OEM_FJ_ROYA");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_FJ_TOUROKU == VK_OEM_FJ_TOUROKU, "VK_OEM_FJ_TOUROKU");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_JUMP == VK_OEM_JUMP, "VK_OEM_JUMP");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_MINUS == VK_OEM_MINUS, "VK_OEM_MINUS");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_PA1 == VK_OEM_PA1, "VK_OEM_PA1");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_PA2 == VK_OEM_PA2, "VK_OEM_PA2");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_PA3 == VK_OEM_PA3, "VK_OEM_PA3");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_PERIOD == VK_OEM_PERIOD, "VK_OEM_PERIOD");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_PLUS == VK_OEM_PLUS, "VK_OEM_PLUS");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_RESET == VK_OEM_RESET, "VK_OEM_RESET");
_Static_assert(OBS_WINDOWS_KEYS_VK_OEM_WSCTRL == VK_OEM_WSCTRL, "VK_OEM_WSCTRL");
_Static_assert(OBS_WINDOWS_KEYS_VK_PA1 == VK_PA1, "VK_PA1");
_Static_assert(OBS_WINDOWS_KEYS_VK_PACKET == VK_PACKET, "VK_PACKET");
_Static_assert(OBS_WINDOWS_KEYS_VK_PAUSE == VK_PAUSE, "VK_PAUSE");
_Static_assert(OBS_WINDOWS_KEYS_VK_PLAY == VK_PLAY, "VK_PLAY");
_Static_assert(OBS_WINDOWS_KEYS_VK_PRINT == VK_PRINT, "VK_PRINT");
_Static_assert(OBS_WINDOWS_KEYS_VK_PRIOR == VK_PRIOR, "VK_PRIOR");
_Static_assert(OBS_WINDOWS_KEYS_VK_PROCESSKEY == VK_PROCESSKEY, "VK_PROCESSKEY");
_Static_assert(OBS_WINDOWS_KEYS_VK_RBUTTON == VK_RBUTTON, "VK_RBUTTON");
_Static_assert(OBS_WINDOWS_KEYS_VK_RCONTROL == VK_RCONTROL, "VK_RCONTROL");
_Static_assert(OBS_WINDOWS_KEYS_VK_RETURN == VK_RETURN, "VK_RETURN");
_Static_assert(OBS_WINDOWS_KEYS_VK_RIGHT == VK_RIGHT, "VK_RIGHT");
_Static_assert(OBS_WINDOWS_KEYS_VK_RMENU == VK_RMENU, "VK_RMENU");
_Static_assert(OBS_WINDOWS_KEYS_VK_RSHIFT == VK_RSHIFT, "VK_RSHIFT");
_Static_assert(OBS_WINDOWS_KEYS_VK_RWIN == VK_RWIN, "VK_RWIN");
_Static_assert(OBS_WINDOWS_KEYS_VK_SCROLL == VK_SCROLL, "VK_SCROLL");
_Static_assert(OBS_WINDOWS_KEYS_VK_SELECT == VK_SELECT, "VK_SELECT");
_Static_assert(OBS_WINDOWS_KEYS_VK_SEPARATOR == VK_SEPARATOR, "VK_SEPARATOR");
_Static_assert(OBS_WINDOWS_KEYS_VK_SHIFT == VK_SHIFT, "VK_SHIFT");
_Static_assert(OBS_WINDOWS_KEYS_VK_SLEEP == VK_SLEEP, "VK_SLEEP");
_Static_assert(OBS_WINDOWS_KEYS_VK_SNAPSHOT == VK_SNAPSHOT, "VK_SNAPSHOT");
_Static_assert(OBS_WINDOWS_KEYS_VK_SPACE == VK_SPACE, "VK_SPACE");
_Static_assert(OBS_WINDOWS_KEYS_VK_SUBTRACT == VK_SUBTRACT, "VK_SUBTRACT");
_Static_assert(OBS_WINDOWS_KEYS_VK_TAB == VK_TAB, "VK_TAB");
_Static_assert(OBS_WINDOWS_KEYS_VK_UP == VK_UP, "VK_UP");
_Static_assert(OBS_WINDOWS_KEYS_VK_VOLUME_DOWN == VK_VOLUME_DOWN, "VK_VOLUME_DOWN");
_Static_assert(OBS_WINDOWS_KEYS_VK_VOLUME_MUTE == VK_VOLUME_MUTE, "VK_VOLUME_MUTE");
_Static_assert(OBS_WINDOWS_KEYS_VK_VOLUME_UP == VK_VOLUME_UP, "VK_VOLUME_UP");
_Static_assert(OBS_WINDOWS_KEYS_VK_XBUTTON1 == VK_XBUTTON1, "VK_XBUTTON1");
_Static_assert(OBS_WINDOWS_KEYS_VK_XBUTTON2 == VK_XBUTTON2, "VK_XBUTTON2");
_Static_assert(OBS_WINDOWS_KEYS_VK_ZOOM == VK_ZOOM, "VK_ZOOM");

bool obs_hotkeys_platform_init(struct obs_core_hotkeys *hotkeys)
{
	hotkeys->platform_context = bzalloc(sizeof(obs_hotkeys_platform_t));

	for (size_t i = 0; i < OBS_KEY_LAST_VALUE; i++)
		hotkeys->platform_context->vk_codes[i] = obs_windows_keys_virtual_key(i);

	return true;
}

void obs_hotkeys_platform_free(struct obs_core_hotkeys *hotkeys)
{
	bfree(hotkeys->platform_context);
	hotkeys->platform_context = NULL;
}

static bool vk_down(DWORD vk)
{
	short state = GetAsyncKeyState(vk);
	bool down = (state & 0x8000) != 0;

	return down;
}

bool obs_hotkeys_platform_is_pressed(obs_hotkeys_platform_t *context, obs_key_t key)
{
	if (obs_windows_keys_either_win(key)) {
		return vk_down(VK_LWIN) || vk_down(VK_RWIN);
	}

	UNUSED_PARAMETER(context);
	return vk_down(obs_key_to_virtual_key(key));
}

void obs_key_to_str(obs_key_t key, struct dstr *str)
{
	wchar_t name[128] = L"";
	UINT scan_code;
	int vk;
	int number = 0;

	switch (obs_windows_keys_name(key, &number)) {
	case OBS_WINDOWS_KEYS_NAME_NONE:
		return;

	case OBS_WINDOWS_KEYS_NAME_F:
		dstr_printf(str, "F%d", number);
		return;
	case OBS_WINDOWS_KEYS_NAME_MOUSE:
		if (obs->hotkeys.translations[key]) {
			dstr_copy(str, obs->hotkeys.translations[key]);
		} else {
			dstr_printf(str, "Mouse %d", number);
		}
		return;
	case OBS_WINDOWS_KEYS_NAME_PAUSE:
		dstr_copy(str, obs_get_hotkey_translation(key, "Pause"));
		return;

	case OBS_WINDOWS_KEYS_NAME_META:
		dstr_copy(str, obs_get_hotkey_translation(key, "Windows"));
		return;

	case OBS_WINDOWS_KEYS_NAME_SYSTEM:
		break;
	}

	vk = obs_key_to_virtual_key(key);
	scan_code = obs_windows_keys_scan_code(vk, MapVirtualKey(vk, 0));

	if (obs_windows_keys_system_name(key, scan_code) && GetKeyNameTextW(scan_code, name, 128) != 0) {
		dstr_from_wcs(str, name);
	} else if (key != OBS_KEY_NONE) {
		dstr_copy(str, obs_key_to_name(key));
	}
}

obs_key_t obs_key_from_virtual_key(int code)
{
	obs_hotkeys_platform_t *platform = obs->hotkeys.platform_context;

	return obs_windows_keys_from_virtual_key(platform->vk_codes, code);
}

/* obs_key_to_virtual_key()'s table, read only for a key other than Meta. */
static const int *windows_vk_codes(void)
{
	return obs->hotkeys.platform_context->vk_codes;
}

int obs_key_to_virtual_key(obs_key_t key)
{
	return obs_windows_keys_to_virtual_key(key, windows_vk_codes);
}

static inline void add_combo_key(obs_key_t key, struct dstr *str)
{
	struct dstr key_str = {0};

	obs_key_to_str(key, &key_str);

	if (!dstr_is_empty(&key_str)) {
		if (!dstr_is_empty(str)) {
			dstr_cat(str, " + ");
		}
		dstr_cat_dstr(str, &key_str);
	}

	dstr_free(&key_str);
}

void obs_key_combination_to_str(obs_key_combination_t combination, struct dstr *str)
{
	obs_key_t keys[OBS_WINDOWS_KEYS_COMBINATION_MAX];
	size_t num = obs_windows_keys_combination(combination.modifiers, combination.key, keys);

	for (size_t i = 0; i < num; i++) {
		add_combo_key(keys[i], str);
	}
}

bool sym_initialize_called = false;

static void reset_win32_symbol_paths(void)
{
	static BOOL(WINAPI * sym_initialize_w)(HANDLE, const wchar_t *, BOOL);
	static BOOL(WINAPI * sym_set_search_path_w)(HANDLE, const wchar_t *);
	static bool funcs_initialized = false;
	static bool initialize_success = false;

	struct obs_module *module = obs->first_module;
	struct dstr path_str = {0};
	DARRAY(char *) paths;
	wchar_t *path_str_w = NULL;
	char *abspath;

	da_init(paths);

	if (!funcs_initialized) {
		HMODULE mod;
		/* GetProcAddress yields a FARPROC and the members are function
		 * pointers: casting between them is C4152 at /W4. Copy each
		 * address through a union of that member's own type instead (as
		 * LOAD_MODULE_SYMBOL does), one union per member because the two
		 * prototypes differ and a shared union would be C4113. */
		union {
			FARPROC proc;
			__typeof__(sym_initialize_w) fn;
		} sym_initialize;
		union {
			FARPROC proc;
			__typeof__(sym_set_search_path_w) fn;
		} sym_search_path;
		funcs_initialized = true;

		mod = LoadLibraryW(L"DbgHelp");
		if (!mod)
			return;

		sym_initialize.proc = GetProcAddress(mod, "SymInitializeW");
		sym_initialize_w = sym_initialize.fn;
		sym_search_path.proc = GetProcAddress(mod, "SymSetSearchPathW");
		sym_set_search_path_w = sym_search_path.fn;
		if (!sym_initialize_w || !sym_set_search_path_w) {
			FreeLibrary(mod);
			return;
		}

		initialize_success = true;
		// Leaks 'mod' once.
	}

	if (!initialize_success)
		return;

	abspath = os_get_abs_path_ptr(".");
	if (abspath)
		da_push_back(paths, &abspath);

	while (module) {
		bool found = false;
		struct dstr path = {0};
		char *path_end;

		dstr_copy(&path, module->bin_path);
		dstr_replace(&path, "/", "\\");

		path_end = strrchr(path.array, '\\');
		if (!path_end) {
			module = module->next;
			dstr_free(&path);
			continue;
		}

		*path_end = 0;

		abspath = os_get_abs_path_ptr(path.array);
		if (abspath) {
			for (size_t i = 0; i < paths.num; i++) {
				const char *existing_path = paths.array[i];
				if (astrcmpi(abspath, existing_path) == 0) {
					found = true;
					break;
				}
			}

			if (!found) {
				da_push_back(paths, &abspath);
			} else {
				bfree(abspath);
			}
		}

		dstr_free(&path);

		module = module->next;
	}

	for (size_t i = 0; i < paths.num; i++) {
		const char *path = paths.array[i];
		if (path && *path) {
			if (i != 0)
				dstr_cat(&path_str, ";");
			dstr_cat(&path_str, paths.array[i]);
		}
	}

	if (path_str.array) {
		os_utf8_to_wcs_ptr(path_str.array, path_str.len, &path_str_w);
		if (path_str_w) {
			if (!sym_initialize_called) {
				sym_initialize_w(GetCurrentProcess(), path_str_w, false);
				sym_initialize_called = true;
			} else {
				sym_set_search_path_w(GetCurrentProcess(), path_str_w);
			}

			bfree(path_str_w);
		}
	}

	for (size_t i = 0; i < paths.num; i++)
		bfree(paths.array[i]);
	dstr_free(&path_str);
	da_free(paths);
}

extern void initialize_crash_handler(void);

void obs_init_win32_crash_handler(void)
{
	initialize_crash_handler();
}

static bool initialize_com(void)
{
	const HRESULT hr = CoInitializeEx(0, COINIT_APARTMENTTHREADED);
	const bool success = SUCCEEDED(hr);
	if (!success)
		blog(LOG_ERROR, "CoInitializeEx failed: 0x%08X", hr);
	return success;
}

static void uninitialize_com(void)
{
	CoUninitialize();
}

/* libobs core host facade (obs-host/, #1051) */

static bool com_initialized = false;

void obs_host_startup(void)
{
	com_initialized = initialize_com();
}

void obs_host_shutdown(void)
{
	if (com_initialized) {
		uninitialize_com();
		com_initialized = false;
	}
}

static const char *reset_win32_symbol_paths_name = "reset_win32_symbol_paths";

void obs_host_post_load_modules(void)
{
	profile_start(reset_win32_symbol_paths_name);
	reset_win32_symbol_paths();
	profile_end(reset_win32_symbol_paths_name);
}

bool obs_host_display_clear_workaround(void)
{
	/* Conservative test for NVIDIA flickering in multi-GPU setups */
	return gs_get_adapter_count() > 1 && !gs_can_adapter_fast_clear();
}

bool obs_host_init_encode_copy_surface(struct obs_core_video_mix *video, uint32_t width, uint32_t height, size_t i)
{
	if (video->using_nv12_tex)
		video->copy_surfaces_encode[i] = gs_stagesurface_create_nv12(width, height);
	else if (video->using_p010_tex)
		video->copy_surfaces_encode[i] = gs_stagesurface_create_p010(width, height);
	else
		return true;
	return video->copy_surfaces_encode[i] != NULL;
}

void obs_host_gpu_encode_surfaces(struct obs_core_video_mix *video, gs_stagesurf_t *const **copy_surfaces,
				  size_t *channel_count)
{
	*copy_surfaces = video->copy_surfaces_encode;
	*channel_count = 1;
}

bool obs_host_nv12_planes_share_texture(void)
{
	return true;
}

void obs_host_tex_frame_acquire(struct obs_tex_frame *tf)
{
	gs_texture_acquire_sync(tf->tex, tf->lock_key, GS_WAIT_INFINITE);
}

void obs_host_tex_frame_release(struct obs_tex_frame *tf)
{
	tf->handle = gs_texture_get_shared_handle(tf->tex);
	gs_texture_release_sync(tf->tex, ++tf->lock_key);
}

uint32_t obs_host_texture_shared_handle(gs_texture_t *tex)
{
	return gs_texture_get_shared_handle(tex);
}

struct winrt_exports {
	void (*winrt_initialize)();
	void (*winrt_uninitialize)();
	struct winrt_disaptcher *(*winrt_dispatcher_init)();
	void (*winrt_dispatcher_free)(struct winrt_disaptcher *dispatcher);
	void (*winrt_capture_thread_start)();
	void (*winrt_capture_thread_stop)();
};

/* os_dlsym returns a void *, and assigning it straight to a function-pointer
 * member is C4152 at /W4, so copy the address through a union, the way
 * GRAPHICS_IMPORT (libobs/graphics/graphics-imports.c.h) does. */
#define WINRT_IMPORT(func)                                        \
	do {                                                      \
		union {                                           \
			void *obj;                                \
			__typeof__(exports->func) fn;             \
		} _sym;                                           \
		_sym.obj = os_dlsym(module, #func);               \
		exports->func = _sym.fn;                          \
		if (!exports->func) {                             \
			success = false;                          \
			blog(LOG_ERROR,                           \
			     "Could not load function '%s' from " \
			     "module '%s'",                       \
			     #func, module_name);                 \
		}                                                 \
	} while (false)

static bool load_winrt_imports(struct winrt_exports *exports, void *module, const char *module_name)
{
	bool success = true;

	WINRT_IMPORT(winrt_initialize);
	WINRT_IMPORT(winrt_uninitialize);
	WINRT_IMPORT(winrt_dispatcher_init);
	WINRT_IMPORT(winrt_dispatcher_free);
	WINRT_IMPORT(winrt_capture_thread_start);
	WINRT_IMPORT(winrt_capture_thread_stop);

	return success;
}

struct winrt_state {
	bool loaded;
	void *winrt_module;
	struct winrt_exports exports;
	struct winrt_disaptcher *dispatcher;
};

/* Owned by the graphics thread: obs_host_graphics_thread_begin and _end run
 * on it, and only one graphics thread runs at a time. */
static struct winrt_state graphics_thread_winrt;

void obs_host_graphics_thread_begin(void)
{
	static const char *const module_name = "libobs-winrt";
	struct winrt_state *winrt = &graphics_thread_winrt;

	winrt->winrt_module = os_dlopen(module_name);
	winrt->loaded = winrt->winrt_module && load_winrt_imports(&winrt->exports, winrt->winrt_module, module_name);
	winrt->dispatcher = NULL;
	if (winrt->loaded) {
		winrt->exports.winrt_initialize();
		winrt->dispatcher = winrt->exports.winrt_dispatcher_init();

		gs_enter_context(obs->video.graphics);
		winrt->exports.winrt_capture_thread_start();
		gs_leave_context();
	}
}

void obs_host_graphics_thread_end(void)
{
	struct winrt_state *winrt = &graphics_thread_winrt;

	if (winrt->winrt_module) {
		if (winrt->loaded) {
			winrt->exports.winrt_capture_thread_stop();
			if (winrt->dispatcher)
				winrt->exports.winrt_dispatcher_free(winrt->dispatcher);
			winrt->exports.winrt_uninitialize();
		}

		os_dlclose(winrt->winrt_module);
	}
	memset(winrt, 0, sizeof(*winrt));
}

void obs_host_pump_messages(void)
{
	MSG msg;
	while (PeekMessage(&msg, NULL, 0, 0, PM_REMOVE)) {
		TranslateMessage(&msg);
		DispatchMessage(&msg);
	}
}

/* The obs-host capabilities this host lacks (#1051): no-op headers, built
 * here rather than as unity group members (BUILD.md). */
#include "libobs/obs-host/noop/default-monitoring-device.hpp"
#include "libobs/obs-host/noop/display-srgb-clear.hpp"
#include "libobs/obs-host/noop/graphics-thread.hpp"
#include "libobs/obs-host/noop/module-paths.hpp"

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef WIN10_GAME_BAR_REG_KEY
#undef WIN10_GAME_DVR_POLICY_REG_KEY
#undef WIN10_GAME_DVR_REG_KEY
#undef WIN10_GAME_MODE_REG_KEY
#undef WINRT_IMPORT
