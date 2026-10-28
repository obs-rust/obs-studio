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

#include "frontend/build/frontend-windows-platform.pch.hpp"

using namespace std;

#ifdef OBS_RUST_VERSION
static bool AppIdentityMatches()
{
	PWSTR actual = nullptr;
	HRESULT result = GetCurrentProcessExplicitAppUserModelID(&actual);
	bool matches = SUCCEEDED(result) && actual && wcscmp(actual, OBS_WINDOWS_APP_ID) == 0;
	CoTaskMemFree(actual);
	return matches;
}
#endif

bool InitializeAppIdentity()
{
#ifdef OBS_RUST_VERSION
	return SUCCEEDED(SetCurrentProcessExplicitAppUserModelID(OBS_WINDOWS_APP_ID)) && AppIdentityMatches();
#else
	return true;
#endif
}

bool VerifyAppIdentity()
{
#ifdef OBS_RUST_VERSION
	if (!AppIdentityMatches()) {
		return false;
	}
	blog(LOG_INFO, "Windows AppUserModelID verified: %s", OBS_DESKTOP_ID);
#endif
	return true;
}

extern bool portable_mode;
extern int GetConfigPath(char *path, size_t size, const char *name);

static inline bool check_path(const char *data, const char *path, string &output)
{
	ostringstream str;
	str << path << data;
	output = str.str();

	blog(LOG_DEBUG, "Attempted path: %s", output.c_str());

	return os_file_exists(output.c_str());
}

bool GetDataFilePath(const char *data, string &output)
{
	if (check_path(data, "data/obs-studio/", output)) {
		return true;
	}

	return check_path(data, OBS_DATA_PATH "/obs-studio/", output);
}

string GetDefaultVideoSavePath()
{
	wchar_t path_utf16[MAX_PATH];
	char path_utf8[MAX_PATH] = {};

	SHGetFolderPathW(NULL, CSIDL_MYVIDEO, NULL, SHGFP_TYPE_CURRENT, path_utf16);

	os_wcs_to_utf8(path_utf16, wcslen(path_utf16), path_utf8, MAX_PATH);
	return string(path_utf8);
}

static vector<string> GetUserPreferredLocales()
{
	vector<string> result;

	ULONG num, length = 0;
	if (!GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &num, nullptr, &length)) {
		return result;
	}

	vector<wchar_t> buffer(length);
	if (!GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &num, &buffer.front(), &length)) {
		return result;
	}

	result.reserve(num);
	auto start = begin(buffer);
	auto end_ = end(buffer);
	decltype(start) separator;
	while ((separator = find(start, end_, 0)) != end_) {
		if (result.size() == num) {
			break;
		}

		char conv[MAX_PATH] = {};
		os_wcs_to_utf8(&*start, separator - start, conv, MAX_PATH);

		result.emplace_back(conv);

		start = separator + 1;
	}

	return result;
}

vector<string> GetPreferredLocales()
{
	vector<string> windows_locales = GetUserPreferredLocales();
	auto obs_locales = GetLocaleNames();
	auto windows_to_obs = [&obs_locales](string windows) {
		string lang_match;

		for (auto &locale_pair : obs_locales) {
			auto &locale = locale_pair.first;
			if (locale == windows.substr(0, locale.size())) {
				return locale;
			}

			if (lang_match.size()) {
				continue;
			}

			if (locale.substr(0, 2) == windows.substr(0, 2)) {
				lang_match = locale;
			}
		}

		return lang_match;
	};

	vector<string> result;
	result.reserve(obs_locales.size());

	for (const string &locale : windows_locales) {
		string match = windows_to_obs(locale);
		if (!match.size()) {
			continue;
		}

		if (find(begin(result), end(result), match) != end(result)) {
			continue;
		}

		result.emplace_back(match);
	}

	return result;
}

uint32_t GetWindowsVersion()
{
	static uint32_t ver = 0;

	if (ver == 0) {
		struct win_version_info ver_info;

		get_win_ver(&ver_info);
		ver = (ver_info.major << 8) | ver_info.minor;
	}

	return ver;
}

uint32_t GetWindowsBuild()
{
	static uint32_t build = 0;

	if (build == 0) {
		struct win_version_info ver_info;

		get_win_ver(&ver_info);
		build = ver_info.build;
	}

	return build;
}

bool IsAlwaysOnTop(QWidget *window)
{
	DWORD exStyle = GetWindowLong((HWND)window->winId(), GWL_EXSTYLE);
	return (exStyle & WS_EX_TOPMOST) != 0;
}

void SetAlwaysOnTop(QWidget *window, bool enable)
{
	HWND hwnd = (HWND)window->winId();
	SetWindowPos(hwnd, enable ? HWND_TOPMOST : HWND_NOTOPMOST, 0, 0, 0, 0,
		     SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
}

void SetProcessPriority(const char *priority)
{
	if (!priority) {
		return;
	}

	if (strcmp(priority, "High") == 0) {
		SetPriorityClass(GetCurrentProcess(), HIGH_PRIORITY_CLASS);
	} else if (strcmp(priority, "AboveNormal") == 0) {
		SetPriorityClass(GetCurrentProcess(), ABOVE_NORMAL_PRIORITY_CLASS);
	} else if (strcmp(priority, "Normal") == 0) {
		SetPriorityClass(GetCurrentProcess(), NORMAL_PRIORITY_CLASS);
	} else if (strcmp(priority, "BelowNormal") == 0) {
		SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS);
	} else if (strcmp(priority, "Idle") == 0) {
		SetPriorityClass(GetCurrentProcess(), IDLE_PRIORITY_CLASS);
	}
}

void SetWin32DropStyle(QWidget *window)
{
	HWND hwnd = (HWND)window->winId();
	LONG_PTR ex_style = GetWindowLongPtr(hwnd, GWL_EXSTYLE);
	ex_style |= WS_EX_ACCEPTFILES;
	SetWindowLongPtr(hwnd, GWL_EXSTYLE, ex_style);
}

bool SetDisplayAffinitySupported(void)
{
	static bool checked = false;
	static bool supported;

	/* this has to be version gated as setting WDA_EXCLUDEFROMCAPTURE on
	   older Windows builds behaves like WDA_MONITOR (black box) */

	if (!checked) {
		if (GetWindowsVersion() > 0x0A00 || GetWindowsVersion() == 0x0A00 && GetWindowsBuild() >= 19041) {
			supported = true;
		} else {
			supported = false;
		}

		checked = true;
	}

	return supported;
}

bool DisableAudioDucking(bool disable)
{
	ComPtr<IMMDeviceEnumerator> devEmum;
	ComPtr<IMMDevice> device;
	ComPtr<IAudioSessionManager2> sessionManager2;
	ComPtr<IAudioSessionControl> sessionControl;
	ComPtr<IAudioSessionControl2> sessionControl2;

	HRESULT result = CoCreateInstance(__uuidof(MMDeviceEnumerator), nullptr, CLSCTX_INPROC_SERVER,
					  __uuidof(IMMDeviceEnumerator), (void **)&devEmum);
	if (FAILED(result)) {
		return false;
	}

	result = devEmum->GetDefaultAudioEndpoint(eRender, eConsole, &device);
	if (FAILED(result)) {
		return false;
	}

	result = device->Activate(__uuidof(IAudioSessionManager2), CLSCTX_INPROC_SERVER, nullptr,
				  (void **)&sessionManager2);
	if (FAILED(result)) {
		return false;
	}

	result = sessionManager2->GetAudioSessionControl(nullptr, 0, &sessionControl);
	if (FAILED(result)) {
		return false;
	}

	result = sessionControl->QueryInterface(&sessionControl2);
	if (FAILED(result)) {
		return false;
	}

	result = sessionControl2->SetDuckingPreference(disable);
	return SUCCEEDED(result);
}

struct RunOnceMutexData {
	WinHandle handle;

	inline RunOnceMutexData(HANDLE h) : handle(h) {}
};

RunOnceMutex::RunOnceMutex(RunOnceMutex &&rom)
{
	delete data;
	data = rom.data;
	rom.data = nullptr;
}

RunOnceMutex::~RunOnceMutex()
{
	delete data;
}

RunOnceMutex &RunOnceMutex::operator=(RunOnceMutex &&rom)
{
	delete data;
	data = rom.data;
	rom.data = nullptr;
	return *this;
}

RunOnceMutex CheckIfAlreadyRunning(bool &already_running)
{
	string name;

	if (!portable_mode) {
		name = OBS_RUN_ONCE_NAME;
	} else {
		char path[500];
		char absPath[512];
		*path = 0;
		*absPath = 0;
		GetAppConfigPath(path, sizeof(path), "");
		os_get_abs_path(path, absPath, sizeof(absPath));
		name = OBS_PORTABLE_RUN_ONCE_NAME;
		name += absPath;
	}

	BPtr<wchar_t> wname;
	os_utf8_to_wcs_ptr(name.c_str(), name.size(), &wname);

	if (wname) {
		wchar_t *temp = wname;
		while (*temp) {
			if (!iswalnum(*temp)) {
				*temp = L'_';
			}
			temp++;
		}
	}

	HANDLE h = OpenMutexW(SYNCHRONIZE, false, wname.Get());
	already_running = !!h;

	if (!already_running) {
		h = CreateMutexW(nullptr, false, wname.Get());
	}

	RunOnceMutex rom(h ? new RunOnceMutexData(h) : nullptr);
	return rom;
}

struct MonitorData {
	const wchar_t *id;
	MONITORINFOEX info;
	bool found;
};

static BOOL CALLBACK GetMonitorCallback(HMONITOR monitor, HDC, LPRECT, LPARAM param)
{
	MonitorData *data = (MonitorData *)param;

	if (GetMonitorInfoW(monitor, &data->info)) {
		if (wcscmp(data->info.szDevice, data->id) == 0) {
			data->found = true;
			return false;
		}
	}

	return true;
}

/* Based on https://www.winehq.org/pipermail/wine-devel/2008-September/069387.html */
typedef const char *(CDECL *WINEGETVERSION)(void);
bool IsRunningOnWine()
{
	WINEGETVERSION func;
	HMODULE nt;

	nt = GetModuleHandleW(L"ntdll");
	if (!nt) {
		return false;
	}

	func = (WINEGETVERSION)GetProcAddress(nt, "wine_get_version");
	if (func) {
		blog(LOG_WARNING, "Running on Wine version \"%s\"", func());
		return true;
	}

	return false;
}

HWND hwnd;
void TaskbarOverlayInit()
{
	hwnd = (HWND)App()->GetMainWindow()->winId();
}

void TaskbarOverlaySetStatus(TaskbarOverlayStatus status)
{
	ITaskbarList4 *taskbarIcon;
	auto hr = CoCreateInstance(CLSID_TaskbarList, NULL, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&taskbarIcon));

	if (FAILED(hr)) {
		taskbarIcon->Release();
		return;
	}

	hr = taskbarIcon->HrInit();

	if (FAILED(hr)) {
		taskbarIcon->Release();
		return;
	}

	QIcon qicon;
	switch (status) {
	case TaskbarOverlayStatusActive:
		qicon = QIcon::fromTheme("obs-active", QIcon(":/res/images/active.png"));
		break;
	case TaskbarOverlayStatusPaused:
		qicon = QIcon::fromTheme("obs-paused", QIcon(":/res/images/paused.png"));
		break;
	case TaskbarOverlayStatusInactive:
		taskbarIcon->SetOverlayIcon(hwnd, nullptr, nullptr);
		taskbarIcon->Release();
		return;
	}

	HICON hicon = nullptr;
	if (!qicon.isNull()) {
		Q_GUI_EXPORT HICON qt_pixmapToWinHICON(const QPixmap &p);
		hicon = qt_pixmapToWinHICON(qicon.pixmap(GetSystemMetrics(SM_CXSMICON)));
		if (!hicon) {
			return;
		}
	}

	taskbarIcon->SetOverlayIcon(hwnd, hicon, nullptr);
	DestroyIcon(hicon);
	taskbarIcon->Release();
}

void *GetGlGetStringProc()
{
	HMODULE gl = GetModuleHandleW(L"opengl32.dll");
	return gl ? reinterpret_cast<void *>(GetProcAddress(gl, "glGetString")) : nullptr;
}

bool HighContrastEnabled()
{
	HIGHCONTRAST hc = {};
	hc.cbSize = sizeof(HIGHCONTRAST);

	if (SystemParametersInfo(SPI_GETHIGHCONTRAST, hc.cbSize, &hc, 0)) {
		return hc.dwFlags & HCF_HIGHCONTRASTON;
	}

	return false;
}

/* OBSApp platform facade (#849) */

// GPU hint exports for AMD/NVIDIA laptops
#ifdef _MSC_VER
extern "C" __declspec(dllexport) DWORD NvOptimusEnablement = 1;
extern "C" __declspec(dllexport) int AmdPowerXpressRequestHighPerformance = 1;
#endif

void InitPlatformConfigDefaults(config_t *appConfig)
{
	config_set_default_string(appConfig, "Video", "Renderer", "Direct3D 11");
	config_set_default_bool(appConfig, "Audio", "DisableAudioDucking", true);
	config_set_default_bool(appConfig, "General", "BrowserHWAccel", true);
}

void ApplyPlatformAppSettings(config_t *appConfig)
{
	bool disableAudioDucking = config_get_bool(appConfig, "Audio", "DisableAudioDucking");
	if (disableAudioDucking) {
		DisableAudioDucking(true);
	}
}

void RevertPlatformAppSettings(config_t *appConfig)
{
	bool disableAudioDucking = config_get_bool(appConfig, "Audio", "DisableAudioDucking");
	if (disableAudioDucking) {
		DisableAudioDucking(false);
	}
}

void LogPlatformSettings(config_t *userConfig)
{
	bool hideFromCapture = config_get_bool(userConfig, "BasicWindow", "HideOBSWindowsFromCapture");
	blog(LOG_INFO, "Hide OBS windows from screen capture: %s", hideFromCapture ? "true" : "false");
}

bool PlatformSupportsBrowserHWAccel()
{
	return true;
}

std::vector<std::string> GetPlatformExtraConfigDirs()
{
	return {"obs-studio/crashes"};
}

void SetApplicationIcon()
{
	QApplication::setWindowIcon(QIcon::fromTheme("obs", QIcon(":/res/images/obs.png")));
}

void InitPlatformDisplay() {}

bool GetPlatformHotkeyOverride(int, obs_key_t &, bool &)
{
	return false;
}

bool SavedVolumeKeysSwapped()
{
	return false;
}

void PlatformLocalTime(const time_t *now, struct tm *out)
{
	localtime_s(out, now);
}

const char *GetPlatformRenderModule(const char *renderer)
{
	return (astrcmpi(renderer, "Direct3D 11") == 0) ? DL_D3D11 : DL_OPENGL;
}

std::string GetPlatformVersionName()
{
	if (sizeof(void *) == 8) {
		return "64-bit, windows";
	}
	return "32-bit, windows";
}

const char *GetPlatformInputAudioSource()
{
	return "wasapi_input_capture";
}

const char *GetPlatformOutputAudioSource()
{
	return "wasapi_output_capture";
}

bool IsPortableModeAllowed()
{
	return true;
}

const char *GetPortableConfigBasePath()
{
	return "../..";
}

bool IsUpdateBranchSupported(bool windows, bool)
{
	return windows;
}

bool PlatformSavesBeforeSignalQuit()
{
	return true;
}

/* OAuth and auto-configuration platform policy (#887). */
bool PlatformSupportsOAuthBrowser()
{
	return true;
}

void ConfigureOAuthLinkMessageBox(QMessageBox &dialog)
{
	UNUSED_PARAMETER(dialog);
}

bool PlatformUseModernWizardStyle()
{
	return true;
}

bool PlatformSupportsAppleHardwareEncoder(const char *id)
{
	UNUSED_PARAMETER(id);
	return false;
}

bool PlatformNeedsQsvStreamingFallback()
{
	return false;
}

/* Settings and hotkey editor platform policy (#869). */
bool PlatformNeedsNonNativeColorDialog()
{
	return false;
}

bool PlatformSupportsSimpleQsvEncoders()
{
	return true;
}

bool PlatformSupportsSimpleAppleEncoders()
{
	return false;
}

bool PlatformHotkeyEditHandlesKeyRelease()
{
	return false;
}

/* Importer platform policy (#881). Translation behavior is retained from
 * frontend/importers/studio.cpp, Copyright (C) 2019-2020 Dillon Pentz. */
size_t PlatformImporterLastSeparator(const std::string &path)
{
	size_t pos = path.find_last_of('\\');
	if (pos == std::string::npos || pos < path.find_last_of('/')) {
		pos = path.find_last_of('/');
	}
	return pos;
}

bool GetPlatformClassicImportDirectory(char *directory, size_t size)
{
	return os_get_config_path(directory, size, "OBS\\sceneCollection\\") != -1;
}

bool GetPlatformStreamlabsImportDirectory(char *directory, size_t size)
{
	return os_get_config_path(directory, size, "slobs-client/SceneCollections/") != -1;
}

bool GetPlatformXSplitImportDirectory(char *directory, size_t size)
{
	return os_get_program_data_path(directory, size, "SplitMediaLabs\\XSplit\\Presentation2.0\\") != -1;
}

void TranslatePlatformImportedSource(json11::Json &value)
{
	using json11::Json;
	Json::object source = value.object_items();
	Json::object settings = source["settings"].object_items();
	std::string id = source["id"].string_value();

#define DirectTranslation(before, after)                                      \
	if (id == before) {                                                   \
		source["id"] = after;                                         \
		source["versioned_id"] = obs_get_latest_input_type_id(after); \
	}

#define ClearTranslation(before, after)                                       \
	if (id == before) {                                                   \
		source["id"] = after;                                         \
		source["settings"] = Json::object{};                          \
		source["versioned_id"] = obs_get_latest_input_type_id(after); \
	}

	DirectTranslation("text_ft2_source", "text_gdiplus");

	ClearTranslation("syphon-input", "game_capture");

	ClearTranslation("coreaudio_input_capture", "wasapi_input_capture");
	ClearTranslation("coreaudio_output_capture", "wasapi_output_capture");
	ClearTranslation("pulse_input_capture", "wasapi_input_capture");
	ClearTranslation("pulse_output_capture", "wasapi_output_capture");

	ClearTranslation("jack_output_capture", "wasapi_output_capture");
	ClearTranslation("alsa_input_capture", "wasapi_input_capture");

	ClearTranslation("av_capture_input", "dshow_input");
	ClearTranslation("v4l2_input", "dshow_input");

	ClearTranslation("xcomposite_input", "window_capture");

	if (id == "monitor_capture" || id == "xshm_input") {
		if (!settings["show_cursor"].is_null()) {
			bool cursor = settings["show_cursor"].bool_value();

			settings["capture_cursor"] = cursor;
		}

		source["id"] = "monitor_capture";
	}

	// Preserve the original final settings assignment, including its legacy
	// interaction with ClearTranslation and null cursor-property lookups.
	source["settings"] = settings;
	value = source;
#undef DirectTranslation
#undef ClearTranslation
}

/* Theme policy (#885). */
const char *GetPlatformThemePrefix()
{
	return "os_win_";
}

QStyle *CreatePlatformThemeStyle()
{
	return new OBSProxyStyle("windowsvista");
}

void ApplyPlatformThemeDarkMode(bool dark)
{
	(void)dark;
}

const char *GetPlatformFallbackTheme(const char *defaultTheme)
{
	return HighContrastEnabled() ? "com.obsproject.System" : defaultTheme;
}

void InstallPlatformAppSignalHandlers() {}
void ConnectPlatformAppSignals(OBSApp *app)
{
	(void)app;
}
bool ConsumePlatformAppSignal(PlatformAppSignal signal)
{
	(void)signal;
	return false;
}
void DisconnectPlatformAppSignals() {}

extern "C" void install_dll_blocklist_hook(void);
extern "C" void log_blocked_dlls(void);
void InstallPlatformDllBlocklist()
{
	install_dll_blocklist_hook();
}
void LogPlatformBlockedDlls()
{
	log_blocked_dlls();
}

/* Shared dialog platform policy (#879). */
const char *GetPlatformAuthorsFile()
{
	return "authors/AUTHORS";
}

Qt::Key GetPlatformFilterRenameKey()
{
	return Qt::Key_F2;
}

void HandlePlatformDialogNativeEvent(QWidget *dialog, void *message)
{
	const MSG &msg = *static_cast<MSG *>(message);
	switch (msg.message) {
	case WM_MOVE:
		for (OBSQTDisplay *const display : dialog->findChildren<OBSQTDisplay *>()) {
			display->OnMove();
		}
		break;
	case WM_DISPLAYCHANGE:
		for (OBSQTDisplay *const display : dialog->findChildren<OBSQTDisplay *>()) {
			display->OnDisplayChange();
		}
	}
}

int TranslatePlatformInteractionModifiers(Qt::KeyboardModifiers mods, bool mouseEvent)
{
	int obsModifiers = INTERACT_NONE;

	if (mods.testFlag(Qt::ShiftModifier)) {
		obsModifiers |= INTERACT_SHIFT_KEY;
	}
	if (mods.testFlag(Qt::AltModifier)) {
		obsModifiers |= INTERACT_ALT_KEY;
	}
	// Handle windows key? Can a browser even trap that key?
	if (mods.testFlag(Qt::ControlModifier)) {
		obsModifiers |= INTERACT_CONTROL_KEY;
	}

	if (!mouseEvent) {
		if (mods.testFlag(Qt::KeypadModifier)) {
			obsModifiers |= INTERACT_IS_KEY_PAD;
		}
	}

	return obsModifiers;
}

const char *GetPlatformPropertiesFontFace()
{
	return "Arial";
}

const char *GetPlatformPropertiesTextSource()
{
	return "text_gdiplus";
}

float GetPlatformPropertiesLabelHeight(uint32_t height)
{
	return height;
}

void AdjustPlatformYouTubeDialogSize(QWidget *dialog)
{
	(void)dialog;
}

/* Component host policy (#877). */
const char *GetPlatformCaptureModuleId(PlatformCaptureKind kind)
{
	switch (kind) {
	case PlatformCaptureKind::Audio:
		return "win-wasapi";
	case PlatformCaptureKind::Display:
		return "win-capture";
	case PlatformCaptureKind::Window:
		return "win-capture";
	}
	return nullptr;
}

const char *GetPlatformCaptureLabelKey(PlatformCaptureKind kind)
{
	switch (kind) {
	case PlatformCaptureKind::Audio:
		return "Device";
	case PlatformCaptureKind::Display:
		return "Monitor";
	case PlatformCaptureKind::Window:
		return "WindowCapture.Window";
	}
	return nullptr;
}

const char *GetPlatformCaptureProperty(PlatformCaptureKind kind, bool &is_int)
{
	(void)is_int;
	const char *prop_name = "device_id";
	if (kind == PlatformCaptureKind::Display) {
		prop_name = "monitor_id";
	} else if (kind == PlatformCaptureKind::Window) {
		prop_name = "window";
	}
	return prop_name;
}

bool PlatformNeedsNonNativeFontDialog()
{
	return false;
}

void ConfigurePlatformSourceItemWidget(QWidget *widget)
{
	(void)widget;
}

bool PlatformEndsSourceEdit()
{
	return false;
}

bool PlatformNeedsDeferredSourceProperties()
{
	BOOL snapEnabled = FALSE;
	SystemParametersInfo(SPI_GETSNAPTODEFBUTTON, 0, &snapEnabled, 0);
	return snapEnabled;
}

QMessageBox::ButtonRole GetPlatformStreamWarningButtonRole(bool settings)
{
	QMessageBox::ButtonRole accept, reject;
	accept = QMessageBox::NoRole;
	reject = QMessageBox::NoRole;
	return settings ? accept : reject;
}

QPalette::ColorGroup GetPlatformVisibilityPaletteGroup(bool active)
{
	QPalette::ColorGroup group = active ? QPalette::Active : QPalette::Inactive;
	return group;
}

QPalette::ColorRole GetPlatformVisibilityHighlightRole()
{
	QPalette::ColorRole highlightRole = QPalette::WindowText;
	return highlightRole;
}

/* Widget graphics handles and policy (#875). */
void ApplyPlatformPreviewTextOutline(obs_data_t *settings)
{
	obs_data_set_int(settings, "outline_color", 0x000000);
	obs_data_set_int(settings, "outline_size", 3);
}

void SetPlatformWidgetIcon(QWidget *widget)
{
	widget->setWindowIcon(QIcon::fromTheme("obs", QIcon(":/res/images/obs.png")));
}

void ConfigurePlatformProjectorWindow(QWidget *widget)
{
	(void)widget;
}

bool FillPlatformGraphicsWindow(QWindow *window, gs_window &gswindow)
{
	gswindow.hwnd = (HWND)window->winId();
	return true;
}

bool PlatformResetsHiddenDisplay()
{
	return false;
}

void HandlePlatformDisplayNativeEvent(OBSQTDisplay *display, void *message)
{
	const MSG &msg = *static_cast<MSG *>(message);
	switch (msg.message) {
	case WM_DISPLAYCHANGE:
		display->OnDisplayChange();
	}
}

void SetOutputNetworkOptions(config_t *config, obs_data_t *settings)
{
	obs_data_set_bool(settings, "new_socket_loop_enabled",
			  config_get_bool(config, "Output", "NewSocketLoopEnable"));
	obs_data_set_bool(settings, "low_latency_mode_enabled", config_get_bool(config, "Output", "LowLatencyEnable"));
}

void TranslateLegacyEncoderId(const char *&)
{
	/* No legacy encoder ids on this host. */
}

void RaiseAfterFileDialog(QWidget *)
{
	/* File dialogs give focus back to their parent window here. */
}

bool UseNativeComboBoxPopup()
{
	return false;
}

static HRESULT SaveJxrImage(LPCWSTR path, uint8_t *pixels, uint32_t cx, uint32_t cy, IWICBitmapFrameEncode *frameEncode,
			    IPropertyBag2 *options)
{
	wchar_t lossless[] = L"Lossless";
	PROPBAG2 bag = {};
	bag.pstrName = lossless;
	VARIANT value = {};
	value.vt = VT_BOOL;
	value.bVal = TRUE;
	HRESULT hr = options->Write(1, &bag, &value);
	if (FAILED(hr)) {
		return hr;
	}

	hr = frameEncode->Initialize(options);
	if (FAILED(hr)) {
		return hr;
	}

	hr = frameEncode->SetSize(cx, cy);
	if (FAILED(hr)) {
		return hr;
	}

	hr = frameEncode->SetResolution(72, 72);
	if (FAILED(hr)) {
		return hr;
	}

	WICPixelFormatGUID pixelFormat = GUID_WICPixelFormat64bppRGBAHalf;
	hr = frameEncode->SetPixelFormat(&pixelFormat);
	if (FAILED(hr)) {
		return hr;
	}

	if (memcmp(&pixelFormat, &GUID_WICPixelFormat64bppRGBAHalf, sizeof(WICPixelFormatGUID)) != 0) {
		return E_FAIL;
	}

	hr = frameEncode->WritePixels(cy, cx * 8, cx * cy * 8, pixels);
	if (FAILED(hr)) {
		return hr;
	}

	hr = frameEncode->Commit();
	if (FAILED(hr)) {
		return hr;
	}

	return S_OK;
}

static HRESULT SaveJxr(LPCWSTR path, uint8_t *pixels, uint32_t cx, uint32_t cy)
{
	Microsoft::WRL::ComPtr<IWICImagingFactory> factory;
	HRESULT hr = CoCreateInstance(CLSID_WICImagingFactory, NULL, CLSCTX_INPROC_SERVER,
				      IID_PPV_ARGS(factory.GetAddressOf()));
	if (FAILED(hr)) {
		return hr;
	}

	Microsoft::WRL::ComPtr<IWICStream> stream;
	hr = factory->CreateStream(stream.GetAddressOf());
	if (FAILED(hr)) {
		return hr;
	}

	hr = stream->InitializeFromFilename(path, GENERIC_WRITE);
	if (FAILED(hr)) {
		return hr;
	}

	Microsoft::WRL::ComPtr<IWICBitmapEncoder> encoder = NULL;
	hr = factory->CreateEncoder(GUID_ContainerFormatWmp, NULL, encoder.GetAddressOf());
	if (FAILED(hr)) {
		return hr;
	}

	hr = encoder->Initialize(stream.Get(), WICBitmapEncoderNoCache);
	if (FAILED(hr)) {
		return hr;
	}

	Microsoft::WRL::ComPtr<IWICBitmapFrameEncode> frameEncode;
	Microsoft::WRL::ComPtr<IPropertyBag2> options;
	hr = encoder->CreateNewFrame(frameEncode.GetAddressOf(), options.GetAddressOf());
	if (FAILED(hr)) {
		return hr;
	}

	hr = SaveJxrImage(path, pixels, cx, cy, frameEncode.Get(), options.Get());
	if (FAILED(hr)) {
		return hr;
	}

	encoder->Commit();
	return S_OK;
}

enum gs_color_space ScreenshotColorSpace(obs_source_t *source)
{
	enum gs_color_space space = obs_source_get_color_space(source, 0, nullptr);
	if (space == GS_CS_709_EXTENDED) {
		/* Convert for JXR */
		space = GS_CS_709_SCRGB;
	}
	return space;
}

bool SaveHdrScreenshot(const char *path, uint8_t *pixels, uint32_t cx, uint32_t cy)
{
	wchar_t *path_w = nullptr;
	os_utf8_to_wcs_ptr(path, 0, &path_w);
	if (!path_w) {
		return false;
	}
	bool saved = SUCCEEDED(SaveJxr(path_w, pixels, cx, cy));
	bfree(path_w);
	return saved;
}

bool IsPlatformImageExtension(const char *suffix)
{
	return astrcmpi(suffix, "jxr") == 0;
}

const char *GetPlatformTextSourceId()
{
	return "text_gdiplus";
}

void SetPlatformTextFileSettings(obs_data_t *settings, const char *file, QString &name)
{
	obs_data_set_bool(settings, "read_from_file", true);
	obs_data_set_string(settings, "file", file);
	name = QUrl::fromLocalFile(QString(file)).fileName();
}

static QString ReadWindowsURLFile(const QString &file)
{
	QSettings iniFile(file, QSettings::IniFormat);
	QVariant url = iniFile.value("InternetShortcut/URL");
	return url.toString();
}

bool ResolvePlatformDroppedFile(QString &file, QFileInfo &info, QString &urlTarget)
{
	if (info.suffix().compare("url", Qt::CaseInsensitive) == 0) {
		urlTarget = ReadWindowsURLFile(file);
		return false;
	}
	if (info.isShortcut()) {
		file = info.symLinkTarget();
		info = QFileInfo(file);
		return info.exists();
	}
	return true;
}

const char *GetPlatformMetaKeyText()
{
	return "Hotkeys.Windows";
}

QString GetPlatformScreenName(QScreen *screen)
{
	return screen->name();
}

bool PlatformUsesDesktopAudioDevice()
{
	return true;
}

QIcon GetPlatformTrayIcon(TrayIconState state)
{
	switch (state) {
	case TrayIconState::Active:
		return QIcon(":/res/images/tray_active.png");
	case TrayIconState::Paused:
		return QIcon(":/res/images/obs_paused.png");
	case TrayIconState::Idle:
		break;
	}
	return QIcon(":/res/images/obs.png");
}

bool PlatformTrayActivationTogglesWindow()
{
	return true;
}

void SetPlatformDockIconVisible(bool) {}

const char *GetPlatformNvencTestProgram()
{
	return "obs-nvenc-test.exe";
}

void ShowPlatformPermissionsDialog(QWidget *) {}

bool PlatformRecreatesWindowsOnTopToggle()
{
	return false;
}

bool PlatformSupportsBuiltinUpdater()
{
	return true;
}

QThread *CreatePlatformUpdateThread(bool manualUpdate, bool repair)
{
	return new AutoUpdateThread(manualUpdate, repair);
}

bool IsWhatsNewItemForPlatform(bool windows, bool, bool)
{
	return windows;
}
