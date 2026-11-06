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
#include "frontend/build/frontend-windows-obs-main.pch.hpp"

/* obs-main's Windows startup and shutdown, crash dialog and help box (#865). */

using namespace std;

#define MAX_CRASH_REPORT_SIZE (200 * 1024)

#define CRASH_MESSAGE                                                      \
	"Woops, OBS has crashed!\n\nWould you like to copy the crash log " \
	"to the clipboard? The crash log will still be saved to:\n\n%s"

static void main_crash_handler(const char *format, va_list args, void * /* param */)
{
	char *text = new char[MAX_CRASH_REPORT_SIZE];

	vsnprintf(text, MAX_CRASH_REPORT_SIZE, format, args);
	text[MAX_CRASH_REPORT_SIZE - 1] = 0;

	string crashFilePath = "obs-studio/crashes";

	delete_oldest_file(true, crashFilePath.c_str());

	string name = crashFilePath + "/";
	name += "Crash " + GenerateTimeDateFilename("txt");

	BPtr<char> path(GetAppConfigPathPtr(name.c_str()));

	fstream file;

	BPtr<wchar_t> wpath;
	os_utf8_to_wcs_ptr(path, 0, &wpath);
	file.open(wpath, ios_base::in | ios_base::out | ios_base::trunc | ios_base::binary);
	file << text;
	file.close();

	string pathString(path.Get());

	std::replace(pathString.begin(), pathString.end(), '/', '\\');

	string absolutePath = canonical(filesystem::path(pathString)).u8string();

	size_t size = static_cast<size_t>(snprintf(nullptr, 0, CRASH_MESSAGE, absolutePath.c_str()));

	unique_ptr<char[]> message_buffer(new char[size + 1]);

	snprintf(message_buffer.get(), size + 1, CRASH_MESSAGE, absolutePath.c_str());

	string finalMessage = string(message_buffer.get(), message_buffer.get() + size);

	int ret = MessageBoxA(NULL, finalMessage.c_str(), "OBS has crashed!", MB_YESNO | MB_ICONERROR | MB_TASKMODAL);

	if (ret == IDYES) {
		size_t len = strlen(text);

		HGLOBAL mem = GlobalAlloc(GMEM_MOVEABLE, len);
		memcpy(GlobalLock(mem), text, len);
		GlobalUnlock(mem);

		OpenClipboard(0);
		EmptyClipboard();
		SetClipboardData(CF_TEXT, mem);
		CloseClipboard();
	}

	exit(-1);
}

static void load_debug_privilege(void)
{
	const DWORD flags = TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY;
	TOKEN_PRIVILEGES tp;
	HANDLE token;
	LUID val;

	if (!OpenProcessToken(GetCurrentProcess(), flags, &token)) {
		return;
	}

	if (!!LookupPrivilegeValue(NULL, SE_DEBUG_NAME, &val)) {
		tp.PrivilegeCount = 1;
		tp.Privileges[0].Luid = val;
		tp.Privileges[0].Attributes = SE_PRIVILEGE_ENABLED;

		AdjustTokenPrivileges(token, false, &tp, sizeof(tp), NULL, NULL);
	}

	if (!!LookupPrivilegeValue(NULL, SE_INC_BASE_PRIORITY_NAME, &val)) {
		tp.PrivilegeCount = 1;
		tp.Privileges[0].Luid = val;
		tp.Privileges[0].Attributes = SE_PRIVILEGE_ENABLED;

		if (!AdjustTokenPrivileges(token, false, &tp, sizeof(tp), NULL, NULL)) {
			blog(LOG_INFO, "Could not set privilege to "
				       "increase GPU priority");
		}
	}

	CloseHandle(token);
}

static constexpr char vcRunErrorTitle[] = "Outdated Visual C++ Runtime";
static constexpr char vcRunErrorMsg[] = "OBS Studio requires a newer version of the Microsoft Visual C++ "
					"Redistributables.\n\nYou will now be directed to the download page.";
static constexpr char vcRunInstallerUrl[] = "https://obsproject.com/visual-studio-2022-runtimes";

static bool vc_runtime_outdated()
{
	win_version_info ver;
	if (!get_dll_ver(L"msvcp140.dll", &ver)) {
		return true;
	}
	/* Major is always 14 (hence 140.dll), so we only care about minor. */
	if (ver.minor >= 40) {
		return false;
	}

	int choice = MessageBoxA(NULL, vcRunErrorMsg, vcRunErrorTitle, MB_OKCANCEL | MB_ICONERROR | MB_TASKMODAL);
	if (choice == IDOK) {
		/* Open the URL in the default browser. */
		ShellExecuteA(NULL, "open", vcRunInstallerUrl, NULL, NULL, SW_SHOWNORMAL);
	}

	return true;
}

static void set_process_mitigation_policies()
{
	PROCESS_MITIGATION_DEP_POLICY dep = {0};
	dep.DisableAtlThunkEmulation = 1;
	dep.Enable = 1;
	dep.Permanent = TRUE;
	SetProcessMitigationPolicy(ProcessDEPPolicy, &dep, sizeof(dep));

	PROCESS_MITIGATION_ASLR_POLICY aslr = {0};
	aslr.EnableBottomUpRandomization = 1;
	aslr.EnableHighEntropy = 1;
	aslr.EnableForceRelocateImages = 1;
	aslr.DisallowStrippedImages = 1;
	SetProcessMitigationPolicy(ProcessASLRPolicy, &aslr, sizeof(aslr));

#ifdef _DEBUG
	PROCESS_MITIGATION_STRICT_HANDLE_CHECK_POLICY hcheck = {0};
	hcheck.RaiseExceptionOnInvalidHandleReference = 1;
	hcheck.HandleExceptionsPermanentlyEnabled = 1;
	SetProcessMitigationPolicy(ProcessStrictHandleCheckPolicy, &hcheck, sizeof(hcheck));
#endif
}
static HMODULE hRtwq = nullptr;

bool obs_main_host_early_init()
{
	// Abort as early as possible if MSVC runtime is outdated
	if (vc_runtime_outdated()) {
		return false;
	}

	// Try to keep this as early as possible
	InstallPlatformDllBlocklist();

	set_process_mitigation_policies();

	obs_init_win32_crash_handler();
	SetErrorMode(SEM_FAILCRITICALERRORS);
	SetSearchPathMode(BASE_SEARCH_PATH_ENABLE_SAFE_SEARCHMODE | BASE_SEARCH_PATH_PERMANENT);
	SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS);
	SetDllDirectoryW(L"");
	load_debug_privilege();
	base_set_crash_handler(main_crash_handler, nullptr);

	/* Shutdown priority value is a range from 0 - 4FF with higher values getting first priority.
	 * 000 - 0FF and 400 - 4FF are reserved system ranges.
	 * Processes start at shutdown level 0x280 by default.
	 * We set the main OBS application to a higher priority to ensure it tries to close before
	 * any subprocesses such as CEF.
	 */
	SetProcessShutdownParameters(0x300, SHUTDOWN_NORETRY);

	hRtwq = LoadLibrary(L"RTWorkQ.dll");
	if (hRtwq) {
		typedef HRESULT(STDAPICALLTYPE * PFN_RtwqStartup)();
		PFN_RtwqStartup func = (PFN_RtwqStartup)GetProcAddress(hRtwq, "RtwqStartup");
		func();
	}

	return true;
}

void obs_main_host_show_help(const string &help)
{
	MessageBoxA(NULL, help.c_str(), "Help", MB_OK | MB_ICONASTERISK);
}

void obs_main_host_shutdown()
{
	if (hRtwq) {
		typedef HRESULT(STDAPICALLTYPE * PFN_RtwqShutdown)();
		PFN_RtwqShutdown func = (PFN_RtwqShutdown)GetProcAddress(hRtwq, "RtwqShutdown");
		func();
		FreeLibrary(hRtwq);
	}

	LogPlatformBlockedDlls();
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef MAX_CRASH_REPORT_SIZE
#undef CRASH_MESSAGE
