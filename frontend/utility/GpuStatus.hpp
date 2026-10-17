#pragma once

#include <QString>
#include <QStringList>

#include <string>

/* GPU acceleration status (obs-rust/obs-studio#846): is OBS rendering on a
 * hardware GPU? The classification lives in rust/obs-gpu-status. */
namespace GpuStatus {

enum class State { Green, Yellow, Red };

struct Info {
	State state = State::Green;
	QString adapter;
	QString summary;
	/* The status bar label ("GPU: 37%", "GPU: n/a", "CPU rendering") and the
	 * tooltip lines below the summary (#1025). */
	QString label;
	QStringList details;
	QStringList causes;
	QStringList gpus;
	/* OBS log lines that explain the state. */
	QStringList logLines;
	/* The gpuAcceleration object obs-websocket returns in GetStats. */
	std::string json;
};

/* False in builds without rust/obs-gpu-status; nothing is shown then. */
bool Available();

/* Offer one OBS log line (from the frontend's log handler). */
void OnLogLine(const char *line);

/* Record libobs' one-second average frame time; call once a second. */
void Sample();

/* Classify the current adapter, detected GPUs and recent frame times. */
Info Query();

/* Call once after video is reset: starts sampling the render GPU's
 * utilization (#1025), logs a warning when the state is yellow or red, and
 * registers the "get_gpu_status" proc on the global proc handler
 * (out: string json) for obs-websocket. */
void Init();

/* Probe the system's GPUs on a thread now, so Init() does not wait for it
 * (nvidia-smi takes tens of milliseconds). */
void Prefetch();

} // namespace GpuStatus
