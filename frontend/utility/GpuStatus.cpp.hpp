#include "frontend/build/frontend-gpu-status.pch.hpp"

namespace GpuStatus {

bool Available()
{
#ifdef OBS_GPU_STATUS
	return true;
#else
	return false;
#endif
}

#ifdef OBS_GPU_STATUS
/* The locale string for `key` with %1, %2, ... filled from `args`, or
 * `english` (from Rust) if the locale has no such key (#945). */
static QString Localized(const QJsonValue &key, const QJsonArray &args, const QString &english)
{
	const char *text = nullptr;
	if (!key.isString() || !App()->TranslateString(key.toString().toUtf8().constData(), &text) || !text) {
		return english;
	}
	QString out = QString::fromUtf8(text);
	for (const QJsonValue arg : args) {
		out = out.arg(arg.toString());
	}
	return out;
}

static Info Parse(int state, const char *json)
{
	Info info;
	info.state = state == 0 ? State::Green : state == 1 ? State::Yellow : State::Red;
	info.json = json ? json : "null";

	QJsonObject obj = QJsonDocument::fromJson(QByteArray(info.json.c_str())).object();
	info.adapter = obj["adapter"].toString();
	info.summary = Localized(obj["summaryKey"], obj["summaryArgs"].toArray(), obj["summary"].toString());
	QJsonObject readout = obj["readout"].toObject();
	info.label = Localized(readout["key"], readout["args"].toArray(), readout["text"].toString());
	for (const QJsonValue value : readout["details"].toArray()) {
		QJsonObject detail = value.toObject();
		info.details << Localized(detail["key"], detail["args"].toArray(), detail["text"].toString());
	}
	QJsonArray causes = obj["causes"].toArray();
	QJsonArray causeKeys = obj["causeKeys"].toArray();
	for (qsizetype i = 0; i < causes.size(); i++) {
		info.causes << Localized(causeKeys.at(i), {}, causes.at(i).toString());
	}
	for (const QJsonValue line : obj["logLines"].toArray()) {
		info.logLines << line.toString();
	}
	for (const QJsonValue value : obj["detectedGpus"].toArray()) {
		QJsonObject gpu = value.toObject();
		QString name = gpu["name"].toString();
		if (name.isEmpty()) {
			name = gpu["vendor"].toString().toUpper() + " " + gpu["deviceId"].toString();
		}
		if (!gpu["renderNode"].toBool()) {
			name += " (no render node)";
		}
		info.gpus << name;
	}
	return info;
}
#endif

#ifdef OBS_GPU_STATUS
typedef const unsigned char *(*GlGetStringFn)(unsigned int);
static constexpr unsigned int GL_VENDOR_ENUM = 0x1F00;
static constexpr unsigned int GL_RENDERER_ENUM = 0x1F01;

struct AdapterSearch {
	uint32_t id;
	std::string name;
};

/* "Loading up D3D11 on adapter <name> (<index>)": the adapter libobs-d3d11
 * opened. Its device_enum_adapters skips Microsoft's Basic Render Driver
 * (WARP), so on a machine rendering with WARP only the log names it. */
static std::mutex loggedD3DAdapterMutex;
static std::string loggedD3DAdapter;

static void RememberD3DAdapter(const char *line)
{
	static constexpr char prefix[] = "Loading up D3D11 on adapter ";
	const char *start = strstr(line, prefix);
	if (!start) {
		return;
	}
	std::string name(start + sizeof(prefix) - 1);
	size_t paren = name.rfind(" (");
	if (paren != std::string::npos) {
		name.resize(paren);
	}
	std::lock_guard<std::mutex> lock(loggedD3DAdapterMutex);
	loggedD3DAdapter = name;
}

static bool FindAdapter(void *param, const char *name, uint32_t id)
{
	auto *search = static_cast<AdapterSearch *>(param);
	if (id != search->id) {
		return true;
	}
	search->name = name ? name : "";
	return false;
}

/* The active adapter as libobs logs it ("Loading up OpenGL on adapter ..."),
 * not gs_get_device_name(), which only names the API ("OpenGL"). Call with
 * the graphics context entered. */
static std::string ActiveAdapterName()
{
	if (gs_get_device_type() == GS_DEVICE_OPENGL) {
		static auto glGetStringFn = reinterpret_cast<GlGetStringFn>(GetGlGetStringProc());
		if (glGetStringFn) {
			const char *vendor = reinterpret_cast<const char *>(glGetStringFn(GL_VENDOR_ENUM));
			const char *renderer = reinterpret_cast<const char *>(glGetStringFn(GL_RENDERER_ENUM));
			if (renderer && *renderer) {
				return vendor && *vendor ? std::string(vendor) + " " + renderer : std::string(renderer);
			}
		}
	} else {
		obs_video_info ovi = {};
		if (obs_get_video_info(&ovi)) {
			AdapterSearch search = {ovi.adapter, ""};
			gs_enum_adapters(FindAdapter, &search);
			if (!search.name.empty()) {
				return search.name;
			}
		}
		std::lock_guard<std::mutex> lock(loggedD3DAdapterMutex);
		if (!loggedD3DAdapter.empty()) {
			return loggedD3DAdapter;
		}
	}
	const char *api = gs_get_device_name();
	return api ? api : "";
}
#endif

#ifdef OBS_GPU_STATUS
/* The active adapter, read once: libobs keeps its graphics device across
 * video resets, and the label refreshes every second, which must not enter
 * graphics against the render thread each time (#1025). */
static std::mutex adapterMutex;
static bool adapterRead = false;
static bool haveAdapter = false;
static std::string adapterName;

/* False when graphics init failed. */
static bool CurrentAdapter(std::string &adapter)
{
	std::lock_guard<std::mutex> lock(adapterMutex);
	if (!adapterRead) {
		obs_enter_graphics();
		haveAdapter = gs_get_device_name() != nullptr;
		if (haveAdapter) {
			adapterName = ActiveAdapterName();
		}
		obs_leave_graphics();
		adapterRead = true;
	}
	adapter = adapterName;
	return haveAdapter;
}

/* Tell the sampler which adapter OBS renders on (#1020): adapter 0 of
 * gs_enum_adapters is the EGL display's render node on Linux, and
 * obs_video_info.adapter the DXGI index on Windows. macOS needs neither. */
static void StartUsageSampler()
{
	AdapterSearch search = {0, ""};
	obs_enter_graphics();
	if (gs_get_device_name() && gs_get_device_type() == GS_DEVICE_OPENGL) {
		gs_enum_adapters(FindAdapter, &search);
	}
	obs_leave_graphics();

	obs_video_info ovi = {};
	int adapterIndex = obs_get_video_info(&ovi) ? (int)ovi.adapter : -1;
	obs_gpu_status_usage_start(search.name.empty() ? nullptr : search.name.c_str(), adapterIndex);
}
#endif

void OnLogLine(const char *line)
{
#ifdef OBS_GPU_STATUS
	RememberD3DAdapter(line);
	obs_gpu_status_log_line(line);
#else
	(void)line;
#endif
}

void Sample()
{
#ifdef OBS_GPU_STATUS
	obs_gpu_status_sample(obs_get_average_frame_time_ns());
#endif
}

Info Query()
{
#ifdef OBS_GPU_STATUS
	std::string adapter;
	bool rendering = CurrentAdapter(adapter);

	int state = 2;
	char *json = obs_gpu_status_report(rendering ? adapter.c_str() : nullptr, obs_get_frame_interval_ns(), &state);
	Info info = Parse(state, json);
	obs_gpu_status_free(json);

	// The sampling thread's log lines (#1022), logged once by whoever asks first.
	if (char *log = obs_gpu_status_take_log()) {
		for (const QString &line : QString::fromUtf8(log).split('\n', Qt::SkipEmptyParts)) {
			blog(LOG_WARNING, "%s", line.toUtf8().constData());
		}
		obs_gpu_status_free(log);
	}
	return info;
#else
	return Info();
#endif
}

#ifdef OBS_GPU_STATUS
static void GetGpuStatusProc(void *, calldata_t *cd)
{
	Info info = Query();
	calldata_set_string(cd, "json", info.json.c_str());
}
#endif

void Prefetch()
{
#ifdef OBS_GPU_STATUS
	obs_gpu_status_prefetch();
#endif
}

void Init()
{
#ifdef OBS_GPU_STATUS
	static bool registered = false;
	if (!registered) {
		proc_handler_add(obs_get_proc_handler(), "void get_gpu_status(out string json)", GetGpuStatusProc,
				 nullptr);
		registered = true;
	}
	StartUsageSampler();

	Info info = Query();
	if (info.state == State::Green) {
		blog(LOG_INFO, "GPU acceleration: green: %s", info.summary.toUtf8().constData());
		return;
	}

	const char *color = info.state == State::Yellow ? "YELLOW" : "RED";
	blog(LOG_WARNING, "==== GPU acceleration: %s ====", color);
	blog(LOG_WARNING, "%s", info.summary.toUtf8().constData());
	for (const QString &gpu : info.gpus) {
		blog(LOG_WARNING, "  detected GPU: %s", gpu.toUtf8().constData());
	}
	for (const QString &cause : info.causes) {
		blog(LOG_WARNING, "  possible cause: %s", cause.toUtf8().constData());
	}
#endif
}

} // namespace GpuStatus
