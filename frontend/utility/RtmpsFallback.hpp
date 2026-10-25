#pragma once

#include <obs-frontend-api.h>
#include <obs.h>

#include <QByteArray>
#include <QString>

#include <functional>
#include <string>
#include <utility>

/* The RTMPS fallback (obs-rust/obs-studio#2205): a stream that rtmp-services
 * upgraded to RTMPS fails to connect, and OBS offers to stream to the same
 * server over plain RTMP. The warning is the shared fallback warning
 * (obs_frontend_warn_fallback, docs/fallback-warning.md): logged at error
 * level and on stderr, a dialog with a three-second countdown that accepts,
 * and Cancel, which stops the start. */
namespace RtmpsFallback {

/* Two choices are open; both keep the behaviour of the first draft of #2219
 * until they are decided.
 *
 * (a) How often to ask. The shared warning shows its dialog once per id in an
 *     OBS session. true: the id names the server, so each server asks once
 *     per session (closest to asking on every failed start without an API
 *     change; a second failure to the same server only logs and continues
 *     over RTMP). false: one id for every server, the helper's default. */
inline constexpr bool IdPerServer = true;

/* (b) The main window hidden (OBS in the tray). The shared warning then logs
 *     and accepts at once. true: OBS also shows a tray notice and waits
 *     HiddenCountdownMs before it restarts over RTMP. false: it restarts at
 *     once. */
inline constexpr bool CountdownWhenHidden = true;
inline constexpr int HiddenCountdownMs = 3000;

inline constexpr const char *BaseId = "rtmp.unencrypted";

/* The fallback warning's id for a stream to plainUrl. */
inline std::string Id(const std::string &plainUrl)
{
	if (!IdPerServer || plainUrl.empty()) {
		return BaseId;
	}
	return std::string(BaseId) + ":" + plainUrl;
}

/* Whether a stream that stopped with code may retry over plain RTMP: only a
 * connection failure of a service stream (multitrack video brings its own
 * URLs), and never when the server's certificate did not verify. That is no
 * network trouble, and plain RTMP would send the stream key unencrypted to a
 * server that could not prove who it is. */
inline bool Allowed(int code, bool multitrack, bool certUnverified)
{
	return code == OBS_OUTPUT_CONNECT_FAILED && !multitrack && !certUnverified;
}

using Answer = std::function<void(bool accepted)>;

/* Raises the shared warning for a fallback to plainUrl. message is translated
 * and has %1 for the URL. answer is called once, on the UI thread: true for
 * the countdown or "Accept anyway" (restart over plain RTMP), false for
 * Cancel or a closed dialog. */
inline void Warn(const QString &title, const QString &message, const std::string &plainUrl, Answer answer)
{
	std::string id = Id(plainUrl);
	QByteArray t = title.toUtf8();
	QByteArray m = message.arg(QString::fromStdString(plainUrl)).toUtf8();
	auto *held = new Answer(std::move(answer));
	obs_frontend_warn_fallback(
		id.c_str(), t.constData(), m.constData(), true,
		[](void *param, bool accepted) {
			Answer *a = static_cast<Answer *>(param);
			if (*a) {
				(*a)(accepted);
			}
			delete a;
		},
		held);
}

} // namespace RtmpsFallback
