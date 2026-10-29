/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>
    Copyright (C) 2014 by Zachary Lund <admin@computerquip.com>

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

#include "frontend/build/frontend-x11.pch.hpp"

using std::ostringstream;
using std::string;
using std::vector;

#ifdef __linux__
void CheckIfAlreadyRunning(bool &already_running)
{
	int uniq = socket(AF_LOCAL, SOCK_DGRAM | SOCK_CLOEXEC, 0);

	if (uniq == -1) {
		blog(LOG_ERROR, "Failed to check for running instance, socket: %d", errno);
		already_running = 0;
		return;
	}

	struct sockaddr_un bindInfo;
	memset(&bindInfo, 0, sizeof(sockaddr_un));
	bindInfo.sun_family = AF_LOCAL;
	auto bindInfoStrlen = snprintf(bindInfo.sun_path + 1, sizeof(bindInfo.sun_path) - 1, "%s %d %s",
				       OBS_INSTANCE_SOCKET_PREFIX, getpid(), App()->GetVersionString().c_str());

	int bindErr = bind(uniq, (struct sockaddr *)&bindInfo,
			   static_cast<socklen_t>(static_cast<int>(sizeof(sa_family_t)) + 1 + bindInfoStrlen));
	already_running = bindErr == 0 ? 0 : 1;

	if (already_running) {
		return;
	}

	FILE *fp = fopen("/proc/net/unix", "re");

	if (fp == NULL) {
		return;
	}

	char *line = NULL;
	size_t n = 0;
	int obsCnt = 0;
	while (getdelim(&line, &n, ' ', fp) != EOF) {
		line[strcspn(line, "\n")] = '\0';
		if (*line == '@') {
			if (strstr(line, "@" OBS_INSTANCE_SOCKET_PREFIX) != NULL) {
				++obsCnt;
			}
		}
	}
	already_running = obsCnt == 1 ? 0 : 1;
	free(line);
	fclose(fp);
}
#endif
#if defined(__FreeBSD__) || defined(__DragonFly__)
struct RunOnce {
	std::thread thr;
	static const char *thr_name;
	std::condition_variable cv;
	std::condition_variable wait_cv;
	std::mutex mtx;
	bool exiting = false;
	bool name_changed = false;

	void thr_proc()
	{
		std::unique_lock<std::mutex> lk(mtx);
		pthread_set_name_np(pthread_self(), thr_name);
		name_changed = true;
		wait_cv.notify_all();
		cv.wait(lk, [this]() { return exiting; });
	}

	~RunOnce()
	{
		if (thr.joinable()) {
			std::unique_lock<std::mutex> lk(mtx);
			exiting = true;
			cv.notify_one();
			lk.unlock();
			thr.join();
		}
	}
} RO;
const char *RunOnce::thr_name = OBS_BSD_THREAD_NAME;

void CheckIfAlreadyRunning(bool &already_running)
{
	std::string tmpfile_name = "/tmp/" OBS_CONFIG_DIRECTORY ".lock." + std::to_string(geteuid());
	int fd = open(tmpfile_name.c_str(), O_RDWR | O_CREAT | O_EXLOCK, 0600);
	if (fd == -1) {
		already_running = true;
		return;
	}

	already_running = false;

	procstat *ps = procstat_open_sysctl();
	unsigned int count;
	auto procs = procstat_getprocs(ps, KERN_PROC_UID | KERN_PROC_INC_THREAD, geteuid(), &count);
	for (unsigned int i = 0; i < count; i++) {
		if (!strncmp(procs[i].ki_tdname, RunOnce::thr_name, sizeof(procs[i].ki_tdname))) {
			already_running = true;
			break;
		}
	}
	procstat_freeprocs(ps, procs);
	procstat_close(ps);

	RO.thr = std::thread(std::mem_fn(&RunOnce::thr_proc), &RO);

	{
		std::unique_lock<std::mutex> lk(RO.mtx);
		RO.wait_cv.wait(lk, []() { return RO.name_changed; });
	}

	unlink(tmpfile_name.c_str());
	close(fd);
}
#endif

static inline bool check_path(const char *data, const char *path, string &output)
{
	ostringstream str;
	str << path << data;
	output = str.str();

	blog(LOG_DEBUG, "Attempted path: %s", output.c_str());

	return (access(output.c_str(), R_OK) == 0);
}

#define INSTALL_DATA_PATH OBS_INSTALL_PREFIX "/" OBS_DATA_PATH "/obs-studio/"

bool GetDataFilePath(const char *data, string &output)
{
	char *data_path = getenv("OBS_DATA_PATH");
	if (data_path != NULL) {
		if (check_path(data, data_path, output)) {
			return true;
		}
	}

	char *relative_data_path = os_get_executable_path_ptr("../" OBS_DATA_PATH "/obs-studio/");

	if (relative_data_path) {
		bool result = check_path(data, relative_data_path, output);
		bfree(relative_data_path);

		if (result) {
			return true;
		}
	}

	if (check_path(data, OBS_DATA_PATH "/obs-studio/", output)) {
		return true;
	}

	if (check_path(data, INSTALL_DATA_PATH, output)) {
		return true;
	}

	return false;
}

string GetDefaultVideoSavePath()
{
	return string(getenv("HOME"));
}

vector<string> GetPreferredLocales()
{
	vector<string> matched;
	string messages = setlocale(LC_MESSAGES, NULL);
	if (!messages.size() || messages == "C" || messages == "POSIX") {
		return {};
	}

	if (messages.size() > 2) {
		messages[2] = '-';
	}

	for (auto &locale_pair : GetLocaleNames()) {
		auto &locale = locale_pair.first;
		if (locale == messages.substr(0, locale.size())) {
			return {locale};
		}

		if (locale.substr(0, 2) == messages.substr(0, 2)) {
			matched.push_back(locale);
		}
	}

	return matched;
}

bool IsAlwaysOnTop(QWidget *window)
{
	return (window->windowFlags() & Qt::WindowStaysOnTopHint) != 0;
}

void SetAlwaysOnTop(QWidget *window, bool enable)
{
	Qt::WindowFlags flags = window->windowFlags();

	if (enable) {
		flags |= Qt::WindowStaysOnTopHint;
	} else {
		flags &= ~Qt::WindowStaysOnTopHint;
	}

	window->setWindowFlags(flags);
	window->show();
}

bool SetDisplayAffinitySupported(void)
{
	// Not implemented yet
	return false;
}

// Not implemented yet
void TaskbarOverlayInit() {}
void TaskbarOverlaySetStatus(TaskbarOverlayStatus) {}

void *GetGlGetStringProc()
{
	/* EGL (X11 and Wayland): eglGetProcAddress returns core functions on
	 * Mesa and NVIDIA. GLVND's libOpenGL and libGL dispatch to the current
	 * context as well. */
	if (void *egl = dlopen("libEGL.so.1", RTLD_LAZY | RTLD_NOLOAD)) {
		typedef void *(*EglGetProcAddressFn)(const char *);
		auto getProc = reinterpret_cast<EglGetProcAddressFn>(dlsym(egl, "eglGetProcAddress"));
		if (void *fn = getProc ? getProc("glGetString") : nullptr) {
			return fn;
		}
	}
	for (const char *lib : {"libOpenGL.so.0", "libGL.so.1"}) {
		if (void *gl = dlopen(lib, RTLD_LAZY)) {
			if (void *fn = dlsym(gl, "glGetString")) {
				return fn;
			}
		}
	}
	return nullptr;
}

bool HighContrastEnabled()
{
	QDBusReply<QVariant> reply;
	QDBusMessage msgXdpSettingsVersion = QDBusMessage::createMethodCall("org.freedesktop.portal.Desktop",
									    "/org/freedesktop/portal/desktop",
									    "org.freedesktop.DBus.Properties", "Get");
	msgXdpSettingsVersion << "org.freedesktop.portal.Settings"
			      << "version";

	reply = QDBusConnection::sessionBus().call(msgXdpSettingsVersion);

	if (!reply.isValid()) {
		blog(LOG_WARNING, "Get on org.freedesktop.portal.Settings returned an invalid reply");
		return false;
	}

	/* NOTE: org.freedesktop.portal.Settings got its contrast settings after
	 * the ReadOne method. So assumes that if ReadOne is not available, contrast
	 * isn't available either. */
	if (uint32_t version = reply.value().toUInt() < 2) {
		blog(LOG_WARNING, "org.freedesktop.portal.Settings version %u does not support ReadOne", version);
		return false;
	}

	/* NOTE: If contrast is not available if will return 0 (false). */
	QDBusMessage msgXdpSettingsContrast =
		QDBusMessage::createMethodCall("org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop",
					       "org.freedesktop.portal.Settings", "ReadOne");

	msgXdpSettingsContrast << "org.freedesktop.appearance"
			       << "contrast";

	reply = QDBusConnection::sessionBus().call(msgXdpSettingsContrast);

	if (!reply.isValid()) {
		blog(LOG_WARNING, "ReadOne on org.freedesktop.portal.Settings returned an invalid reply");
		return false;
	}

	return reply.value().toUInt() != 0;
}

/* OBSApp platform facade (#849) */

void InitPlatformConfigDefaults(config_t *appConfig)
{
	config_set_default_string(appConfig, "Video", "Renderer", "OpenGL");

#ifdef __linux__
	config_set_default_bool(appConfig, "General", "BrowserHWAccel", true);
#endif
}

void ApplyPlatformAppSettings(config_t *) {}

void RevertPlatformAppSettings(config_t *) {}

void LogPlatformSettings(config_t *) {}

bool PlatformSupportsBrowserHWAccel()
{
#ifdef __linux__
	return true;
#else
	return false;
#endif
}

std::vector<std::string> GetPlatformExtraConfigDirs()
{
	return {};
}

bool InitializeAppIdentity()
{
	return true;
}

void SetApplicationIcon()
{
	QApplication::setWindowIcon(QIcon::fromTheme("obs", QIcon(":/res/images/obs.png")));
}

void InitPlatformDisplay()
{
	if (QApplication::platformName() == "xcb") {
		auto native = qGuiApp->nativeInterface<QNativeInterface::QX11Application>();

		obs_set_nix_platform_display(native->display());

		obs_set_nix_platform(OBS_NIX_PLATFORM_X11_EGL);

		blog(LOG_INFO, "Using EGL/X11");
	}

#ifdef ENABLE_WAYLAND
	if (QApplication::platformName().contains("wayland")) {
		auto native = qGuiApp->nativeInterface<QNativeInterface::QWaylandApplication>();

		obs_set_nix_platform_display(native->display());

		obs_set_nix_platform(OBS_NIX_PLATFORM_WAYLAND);
		QApplication::setAttribute(Qt::AA_DontCreateNativeWidgetSiblings);

		blog(LOG_INFO, "Platform: Wayland");
	}
#endif
}

bool GetPlatformHotkeyOverride(int, obs_key_t &, bool &)
{
	return false;
}

bool SavedVolumeKeysSwapped()
{
	return true;
}

void PlatformLocalTime(const time_t *now, struct tm *out)
{
	localtime_r(now, out);
}

const char *GetPlatformRenderModule(const char *)
{
	return DL_OPENGL;
}

std::string GetPlatformVersionName()
{
#if defined(__OpenBSD__)
	return "openbsd";
#elif defined(__FreeBSD__)
	return "freebsd";
#else /* assume linux for the time being */
	return "linux";
#endif
}

const char *GetPlatformInputAudioSource()
{
	return "pulse_input_capture";
}

const char *GetPlatformOutputAudioSource()
{
	return "pulse_output_capture";
}

bool IsPortableModeAllowed()
{
#ifdef ENABLE_PORTABLE_CONFIG
	return true;
#else
	return false;
#endif
}

const char *GetPortableConfigBasePath()
{
#ifdef __linux__
	return "..";
#else
	return "../..";
#endif
}

bool IsUpdateBranchSupported(bool, bool)
{
	return true;
}

bool PlatformSavesBeforeSignalQuit()
{
	return true;
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef INSTALL_DATA_PATH

/* OAuth and auto-configuration platform policy (#887). */
bool PlatformSupportsOAuthBrowser()
{
	return !QApplication::platformName().contains("wayland");
}

void ConfigureOAuthLinkMessageBox(QMessageBox &dialog)
{
	UNUSED_PARAMETER(dialog);
}

bool PlatformUseModernWizardStyle()
{
	return false;
}

bool PlatformSupportsAppleHardwareEncoder(const char *id)
{
	UNUSED_PARAMETER(id);
	return false;
}

bool PlatformNeedsQsvStreamingFallback()
{
#ifdef __linux__
	return true;
#else
	return false;
#endif
}

/* Settings and hotkey editor platform policy (#869). */
bool PlatformNeedsNonNativeColorDialog()
{
#ifdef __linux__
	// Native color dialogs can hang on Ubuntu.
	return true;
#else
	return false;
#endif
}

bool PlatformSupportsSimpleQsvEncoders()
{
	return false;
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
	return path.find_last_of('/');
}

bool GetPlatformClassicImportDirectory(char *, size_t)
{
	return false;
}

bool GetPlatformStreamlabsImportDirectory(char *, size_t)
{
	return false;
}

bool GetPlatformXSplitImportDirectory(char *, size_t)
{
	return false;
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

	DirectTranslation("text_gdiplus", "text_ft2_source");

	ClearTranslation("coreaudio_input_capture", "pulse_input_capture");
	ClearTranslation("coreaudio_output_capture", "pulse_output_capture");
	ClearTranslation("wasapi_input_capture", "pulse_input_capture");
	ClearTranslation("wasapi_output_capture", "pulse_output_capture");

	ClearTranslation("av_capture_input", "v4l2_input");
	ClearTranslation("dshow_input", "v4l2_input");

	if (obs_get_nix_platform() == OBS_NIX_PLATFORM_X11_EGL) {
		ClearTranslation("game_capture", "xcomposite_input");
		ClearTranslation("window_capture", "xcomposite_input");
	} else {
		ClearTranslation("game_capture", "pipewire-screen-capture-source");
		ClearTranslation("window_capture", "pipewire-screen-capture-source");
	}

	if (id == "monitor_capture") {
		source["id"] = "xshm_input";

		if (settings["show_cursor"].is_null() && !settings["capture_cursor"].is_null()) {
			bool cursor = settings["capture_cursor"].bool_value();

			settings["show_cursor"] = cursor;
		}
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
	return "os_lin_";
}

QStyle *CreatePlatformThemeStyle()
{
	return new OBSProxyStyle("Fusion");
}

void ApplyPlatformThemeDarkMode(bool dark)
{
	(void)dark;
}

const char *GetPlatformFallbackTheme(const char *defaultTheme)
{
	return defaultTheme;
}

void InstallPlatformDllBlocklist() {}
void LogPlatformBlockedDlls() {}

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
	(void)dialog;
	(void)message;
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
	return "Monospace";
}

const char *GetPlatformPropertiesTextSource()
{
	return "text_ft2_source";
}

float GetPlatformPropertiesLabelHeight(uint32_t height)
{
	return static_cast<float>(height * 0.8);
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
		return "linux-pulseaudio";
	case PlatformCaptureKind::Display:
		return "linux-capture";
	case PlatformCaptureKind::Window:
		return "linux-capture";
	}
	return nullptr;
}

const char *GetPlatformCaptureLabelKey(PlatformCaptureKind kind)
{
	switch (kind) {
	case PlatformCaptureKind::Audio:
		return "Device";
	case PlatformCaptureKind::Display:
		return "Screen";
	case PlatformCaptureKind::Window:
		return "Window";
	}
	return nullptr;
}

const char *GetPlatformCaptureProperty(PlatformCaptureKind kind, bool &is_int)
{
	const char *prop_name = "device_id";
	if (kind == PlatformCaptureKind::Display) {
		is_int = true;
		prop_name = "screen";
	} else if (kind == PlatformCaptureKind::Window) {
		prop_name = "capture_window";
	}
	return prop_name;
}

bool PlatformNeedsNonNativeFontDialog()
{
	return true;
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
	return false;
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
	(void)active;
	QPalette::ColorGroup group = QPalette::Active;
	return group;
}

QPalette::ColorRole GetPlatformVisibilityHighlightRole()
{
	QPalette::ColorRole highlightRole = QPalette::HighlightedText;
	return highlightRole;
}

/* Widget graphics handles and policy (#875). */
void ApplyPlatformPreviewTextOutline(obs_data_t *settings)
{
	(void)settings;
}

void SetPlatformWidgetIcon(QWidget *widget)
{
	widget->setWindowIcon(QIcon::fromTheme("obs", QIcon(":/res/images/obs.png")));
}

void ConfigurePlatformProjectorWindow(QWidget *widget)
{
	(void)widget;
#if defined(__linux__) || defined(__FreeBSD__) || defined(__DragonFly__)
	// Prevents resizing of projector windows
	widget->setAttribute(Qt::WA_PaintOnScreen, false);
#endif
}

bool FillPlatformGraphicsWindow(QWindow *window, gs_window &gswindow)
{
	bool success = true;
	switch (obs_get_nix_platform()) {
	case OBS_NIX_PLATFORM_X11_EGL:
		gswindow.id = static_cast<uint32_t>(window->winId());
		gswindow.display = obs_get_nix_platform_display();
		break;
#ifdef ENABLE_WAYLAND
	case OBS_NIX_PLATFORM_WAYLAND: {
		gswindow.display = (void *)window->winId();
		success = gswindow.display != nullptr;
		break;
	}
#endif
	default:
		success = false;
		break;
	}
	return success;
}

bool PlatformResetsHiddenDisplay()
{
	return true;
}

void HandlePlatformDisplayNativeEvent(OBSQTDisplay *display, void *message)
{
	(void)display;
	(void)message;
}

void SetOutputNetworkOptions(config_t *, obs_data_t *)
{
	/* No host-specific streaming output options. */
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

enum gs_color_space ScreenshotColorSpace(obs_source_t *)
{
	/* Tonemap to SDR if HDR */
	return GS_CS_SRGB;
}

bool SaveHdrScreenshot(const char *, uint8_t *, uint32_t, uint32_t)
{
	/* No HDR image format here: screenshots are rendered in sRGB. */
	return false;
}

bool IsPlatformImageExtension(const char *)
{
	return false;
}

const char *GetPlatformTextSourceId()
{
	return "text_ft2_source";
}

void SetPlatformTextFileSettings(obs_data_t *settings, const char *file, QString &)
{
	obs_data_set_bool(settings, "from_file", true);
	obs_data_set_string(settings, "text_file", file);
}

bool ResolvePlatformDroppedFile(QString &, QFileInfo &, QString &)
{
	return true;
}

const char *GetPlatformMetaKeyText()
{
	return "Hotkeys.Super";
}

QString GetPlatformScreenName(QScreen *screen)
{
	QString name = screen->model().simplified();

	if (name.length() > 1 && name.endsWith("-")) {
		name.chop(1);
	}
	return name;
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

/* The Rust brand installs it as obs-studio-rust-nvenc-test (#2296). */
#ifndef OBS_NVENC_TEST_NAME
#define OBS_NVENC_TEST_NAME "obs-nvenc-test"
#endif

const char *GetPlatformNvencTestProgram()
{
	return OBS_NVENC_TEST_NAME;
}

void ShowPlatformPermissionsDialog(QWidget *) {}

bool PlatformRecreatesWindowsOnTopToggle()
{
	return true;
}

bool PlatformSupportsBuiltinUpdater()
{
	return false;
}

QThread *CreatePlatformUpdateThread(bool, bool)
{
	return nullptr;
}

bool IsWhatsNewItemForPlatform(bool, bool, bool onLinux)
{
	return onLinux;
}

#undef OBS_NVENC_TEST_NAME
