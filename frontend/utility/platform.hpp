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

#pragma once

#include <util/c99defs.h>
#include <util/config-file.h>
#include <obs.h>
#include <QtCore/Qt>
#include <QMessageBox>
#include <QPalette>

#include <ctime>
#include <string>
#include <vector>

class QWidget;

/* Features the shared frontend code compiles in. Defined here once so the
 * shared files do not test platform macros themselves (#847). */
#if defined(_WIN32) || defined(ENABLE_SPARKLE_UPDATER)
#define OBS_PLATFORM_HAS_UPDATE_BRANCHES 1
#endif

#ifdef __APPLE__
bool InitializeMacApplicationIcon();
bool VerifyMacApplicationIcon();
#endif

/* Gets the path of obs-studio specific data files (such as locale) */
bool GetDataFilePath(const char *data, std::string &path);

std::string GetDefaultVideoSavePath();

std::vector<std::string> GetPreferredLocales();

bool IsAlwaysOnTop(QWidget *window);
void SetAlwaysOnTop(QWidget *window, bool enable);

bool SetDisplayAffinitySupported(void);

bool HighContrastEnabled();

/* Copies the host's own streaming output options from the profile's Output
 * section into the output's settings (Windows: the new socket loop and
 * low-latency mode). */
void SetOutputNetworkOptions(config_t *config, obs_data_t *settings);

/* Replaces an encoder id an older OBS on this host saved in a profile with
 * the id the encoder has now (macOS: the VideoToolbox H.264 encoders). */
void TranslateLegacyEncoderId(const char *&encoder);

/* Hotkeys saved here before #2635 name Volume Up OBS_KEY_VK_VOLUME_DOWN and
 * Volume Down OBS_KEY_VK_VOLUME_UP (Linux key tables). */
bool SavedVolumeKeysSwapped();

/* Brings window back to the front after a file dialog closes (macOS:
 * QTBUG-42661 leaves it behind other windows). */
void RaiseAfterFileDialog(QWidget *window);

/* Whether combo boxes open the host's native popup (macOS) instead of Qt's. */
bool UseNativeComboBoxPopup();

/* The color space a screenshot of source is rendered in: the host keeps HDR
 * when it can save it (Windows: scRGB for JXR), otherwise SDR sRGB. */
enum gs_color_space ScreenshotColorSpace(obs_source_t *source);

/* Saves cx x cy RGBA half-float pixels (a screenshot rendered in a non-sRGB
 * ScreenshotColorSpace()) to path. False when the host cannot (no HDR image
 * format) or saving failed. */
bool SaveHdrScreenshot(const char *path, uint8_t *pixels, uint32_t cx, uint32_t cy);

/* glGetString from the GL library libobs-opengl uses (the frontend does not
 * link GL), or nullptr. Call it only with the graphics context entered. */
void *GetGlGetStringProc();

/* OBSApp platform facade (#849). Each function has one implementation in
 * platform-osx.mm, platform-windows.cpp and platform-x11.cpp. */

/* Platform defaults of the global (app) config: renderer, audio ducking,
 * browser acceleration, macOS vsync. */
void InitPlatformConfigDefaults(config_t *appConfig);
/* Applies platform settings from the app config at startup / reverts them at shutdown. */
void ApplyPlatformAppSettings(config_t *appConfig);
void RevertPlatformAppSettings(config_t *appConfig);
/* Per-platform lines of the startup log. */
void LogPlatformSettings(config_t *userConfig);
bool PlatformSupportsBrowserHWAccel();
/* Directories below the app config path that exist only on this platform. */
std::vector<std::string> GetPlatformExtraConfigDirs();
/* Process identity (Windows AppUserModelID); true when there is nothing to do. */
bool InitializeAppIdentity();
void SetApplicationIcon();
/* Native display setup (X11/Wayland platform selection, macOS widget attribute). */
void InitPlatformDisplay();
/* Hotkeys Qt reports in a platform-specific way (macOS CapsLock). */
bool GetPlatformHotkeyOverride(int qtKey, obs_key_t &key, bool &pressed);
void PlatformLocalTime(const time_t *now, struct tm *out);
/* The graphics module (DL_*) for the configured "Video/Renderer" value. */
const char *GetPlatformRenderModule(const char *renderer);
/* "windows", "mac", "linux", ... as shown in the version string. */
std::string GetPlatformVersionName();
const char *GetPlatformInputAudioSource();
const char *GetPlatformOutputAudioSource();
bool IsPortableModeAllowed();
const char *GetPortableConfigBasePath();
/* Whether an update branch announced for these platforms applies here. */
bool IsUpdateBranchSupported(bool windows, bool macos);
/* Whether SIGINT/SIGTERM/... save the scene collection before quitting (not on macOS). */
bool PlatformSavesBeforeSignalQuit();

/* OAuth and auto-configuration facade (#887). */
class QMessageBox;
bool PlatformSupportsOAuthBrowser();
void ConfigureOAuthLinkMessageBox(QMessageBox &dialog);
bool PlatformUseModernWizardStyle();
bool PlatformSupportsAppleHardwareEncoder(const char *id);
bool PlatformNeedsQsvStreamingFallback();

/* Files dropped on the main window (#873). */
class QFileInfo;
bool IsPlatformImageExtension(const char *suffix);
const char *GetPlatformTextSourceId();
void SetPlatformTextFileSettings(obs_data_t *settings, const char *file, QString &name);
/* False when the drop is handled here or skipped; a non-empty urlTarget is
 * a URL to offer instead. May replace a shortcut with its target. */
bool ResolvePlatformDroppedFile(QString &file, QFileInfo &info, QString &urlTarget);

/* Main window host choices (#873). */
class QScreen;
/* The locale key of the meta (Windows/Super/Command) key's name. */
const char *GetPlatformMetaKeyText();
/* A screen's name for the projector menu, before simplification. */
QString GetPlatformScreenName(QScreen *screen);
/* False where a better desktop audio source replaces the device one. */
bool PlatformUsesDesktopAudioDevice();

/* System tray (#873). */
class QIcon;
enum class TrayIconState { Idle, Active, Paused };
/* The tray image when the icon theme has none. */
QIcon GetPlatformTrayIcon(TrayIconState state);
/* False where a tray click only opens the menu. */
bool PlatformTrayActivationTogglesWindow();
void SetPlatformDockIconVisible(bool visible);

/* The NVENC capability test program's file name, or nullptr where the host
 * has no NVENC (#873). */
const char *GetPlatformNvencTestProgram();

/* Main window controls (#873). */
/* The screen, camera, microphone and input permission dialog, where the
 * host asks for them. */
void ShowPlatformPermissionsDialog(QWidget *parent);
/* True where an always-on-top toggle recreates every window. */
bool PlatformRecreatesWindowsOnTopToggle();

/* Updates (#873). */
class QThread;
/* True where OBS updates itself (the Windows updater), not through Sparkle
 * or a package manager. */
bool PlatformSupportsBuiltinUpdater();
/* The built-in updater's check (or repair) thread; nullptr where there is
 * none. */
QThread *CreatePlatformUpdateThread(bool manualUpdate, bool repair);
/* Whether a What's New item that lists its platforms applies to this host. */
bool IsWhatsNewItemForPlatform(bool onWindows, bool onMacos, bool onLinux);

/* Settings and hotkey editor platform policy (#869). */
bool PlatformNeedsNonNativeColorDialog();
bool PlatformSupportsSimpleQsvEncoders();
bool PlatformSupportsSimpleAppleEncoders();
bool PlatformHotkeyEditHandlesKeyRelease();

/* Importer platform policy (#881). */
namespace json11 {
class Json;
}
size_t PlatformImporterLastSeparator(const std::string &path);
bool GetPlatformClassicImportDirectory(char *directory, size_t size);
bool GetPlatformStreamlabsImportDirectory(char *directory, size_t size);
bool GetPlatformXSplitImportDirectory(char *directory, size_t size);
void TranslatePlatformImportedSource(json11::Json &source);

enum TaskbarOverlayStatus {
	TaskbarOverlayStatusInactive,
	TaskbarOverlayStatusActive,
	TaskbarOverlayStatusPaused,
};
void TaskbarOverlayInit();
void TaskbarOverlaySetStatus(TaskbarOverlayStatus status);

#ifdef _WIN32
class RunOnceMutex;
RunOnceMutex
#else
void
#endif
CheckIfAlreadyRunning(bool &already_running);

#ifdef _WIN32
bool VerifyAppIdentity();
uint32_t GetWindowsVersion();
uint32_t GetWindowsBuild();
void SetProcessPriority(const char *priority);
void SetWin32DropStyle(QWidget *window);
bool DisableAudioDucking(bool disable);

struct RunOnceMutexData;

class RunOnceMutex {
	RunOnceMutexData *data = nullptr;

public:
	RunOnceMutex(RunOnceMutexData *data_) : data(data_) {}
	RunOnceMutex(const RunOnceMutex &rom) = delete;
	RunOnceMutex(RunOnceMutex &&rom);
	~RunOnceMutex();

	RunOnceMutex &operator=(const RunOnceMutex &rom) = delete;
	RunOnceMutex &operator=(RunOnceMutex &&rom);
};

bool IsRunningOnWine();
#endif

#ifdef __APPLE__
typedef enum {
	kAudioDeviceAccess = 0,
	kVideoDeviceAccess = 1,
	kScreenCapture = 2,
	kInputMonitoring = 3
} MacPermissionType;

typedef enum {
	kPermissionNotDetermined = 0,
	kPermissionRestricted = 1,
	kPermissionDenied = 2,
	kPermissionAuthorized = 3,
} MacPermissionStatus;

void EnableOSXVSync(bool enable);
void EnableOSXDockIcon(bool enable);
bool isInBundle();
void InstallNSApplicationSubclass();
void InstallNSThreadLocks();
void disableColorSpaceConversion(QWidget *window);
void SetMacOSDarkMode(bool dark);

MacPermissionStatus CheckPermissionWithPrompt(MacPermissionType type, bool prompt_for_permission);
#define CheckPermission(x) CheckPermissionWithPrompt(x, false)
#define RequestPermission(x) CheckPermissionWithPrompt(x, true)
void OpenMacOSPrivacyPreferences(const char *tab);
#endif

/* Theme policy and application signal transport (#885). */
class QStyle;
class OBSApp;
enum class PlatformAppSignal { Interrupt, Terminate, Abort, Quit };
const char *GetPlatformThemePrefix();
QStyle *CreatePlatformThemeStyle();
void ApplyPlatformThemeDarkMode(bool dark);
const char *GetPlatformFallbackTheme(const char *defaultTheme);
void InstallPlatformAppSignalHandlers();
void ConnectPlatformAppSignals(OBSApp *app);
bool ConsumePlatformAppSignal(PlatformAppSignal signal);
void DisconnectPlatformAppSignals();
void InstallPlatformDllBlocklist();
void LogPlatformBlockedDlls();

/* Shared dialog policy (#879). Interaction Meta handling intentionally differs
 * from the generic Qt wrapper on non-macOS hosts. */
const char *GetPlatformAuthorsFile();
Qt::Key GetPlatformFilterRenameKey();
void HandlePlatformDialogNativeEvent(QWidget *dialog, void *message);
int TranslatePlatformInteractionModifiers(Qt::KeyboardModifiers mods, bool mouseEvent);
const char *GetPlatformPropertiesFontFace();
const char *GetPlatformPropertiesTextSource();
float GetPlatformPropertiesLabelHeight(uint32_t height);
void AdjustPlatformYouTubeDialogSize(QWidget *dialog);

/* Capture toolbar and source component policy (#877). */
enum class PlatformCaptureKind { Audio, Display, Window };
const char *GetPlatformCaptureModuleId(PlatformCaptureKind kind);
const char *GetPlatformCaptureLabelKey(PlatformCaptureKind kind);
/* Only sets isInt where the original host policy did; otherwise preserves it. */
const char *GetPlatformCaptureProperty(PlatformCaptureKind kind, bool &isInt);
bool PlatformNeedsNonNativeFontDialog();
void ConfigurePlatformSourceItemWidget(QWidget *widget);
bool PlatformEndsSourceEdit();
/* Query snap-to-default immediately before scheduling/opening properties. */
bool PlatformNeedsDeferredSourceProperties();
QMessageBox::ButtonRole GetPlatformStreamWarningButtonRole(bool settings);
QPalette::ColorGroup GetPlatformVisibilityPaletteGroup(bool active);
QPalette::ColorRole GetPlatformVisibilityHighlightRole();

/* Native window/display policy (#875). gs_window handles remain borrowed. */
class QWindow;
class OBSQTDisplay;
struct gs_window;
void ApplyPlatformPreviewTextOutline(obs_data_t *settings);
void SetPlatformWidgetIcon(QWidget *widget);
void ConfigurePlatformProjectorWindow(QWidget *widget);
bool FillPlatformGraphicsWindow(QWindow *window, gs_window &gswindow);
bool PlatformResetsHiddenDisplay();
void HandlePlatformDisplayNativeEvent(OBSQTDisplay *display, void *message);
