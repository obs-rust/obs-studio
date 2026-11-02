#pragma once

#include <obs-frontend-api.h>
#include <obs.h>

#include <QByteArray>
#include <QString>

#include <cstring>
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

/* How often to ask: the shared warning shows its dialog once per id in an
 * OBS session, and the id names the server, so each server asks once per
 * session. A second failure to the same server only logs and continues over
 * RTMP. */
inline constexpr bool IdPerServer = true;

/* With the main window hidden (OBS in the tray) the shared warning logs and
 * accepts at once; OBS then shows a tray notice and waits HiddenCountdownMs
 * before it restarts over RTMP. */
inline constexpr bool CountdownWhenHidden = true;
inline constexpr int HiddenCountdownMs = 3000;

/* Why the RTMPS connection failed, as the RTMP output's rtmp_tls_failure proc
 * reports it. */
enum class Cause {
	Connection,     /* timeout, refused, reset, other handshake failure */
	CertUnverified, /* "cert_unverified": the server's certificate */
	NoRootCerts,    /* "no_root_certs": the system has no trusted roots */
};

inline Cause ParseCause(const char *failure)
{
	if (!failure) {
		return Cause::Connection;
	}
	if (strcmp(failure, "cert_unverified") == 0) {
		return Cause::CertUnverified;
	}
	if (strcmp(failure, "no_root_certs") == 0) {
		return Cause::NoRootCerts;
	}
	return Cause::Connection;
}

inline constexpr const char *BaseId = "rtmp.unencrypted";
inline constexpr const char *NoRootCertsId = "rtmp.unencrypted.nocerts";

/* The fallback warning's id for a stream to plainUrl. The missing root
 * certificates have their own id, so that warning is shown once per server
 * even when an ordinary failure was shown first. */
inline std::string Id(const std::string &plainUrl, Cause cause = Cause::Connection)
{
	std::string base = cause == Cause::NoRootCerts ? NoRootCertsId : BaseId;
	if (!IdPerServer || plainUrl.empty()) {
		return base;
	}
	return base + ":" + plainUrl;
}

/* The locale key of the warning's message (%1 is the plain RTMP URL). */
inline const char *MessageKey(Cause cause)
{
	return cause == Cause::NoRootCerts ? "Output.RtmpsFallback.NoRootCerts" : "Output.RtmpsFallback.Message";
}

/* Whether a stream that stopped with code may retry over plain RTMP: only a
 * connection failure of a service stream (multitrack video brings its own
 * URLs), and never when the server's certificate did not verify. That is no
 * network trouble, and plain RTMP would send the stream key unencrypted to a
 * server that could not prove who it is. */
inline bool Allowed(int code, bool multitrack, Cause cause)
{
	return code == OBS_OUTPUT_CONNECT_FAILED && !multitrack && cause != Cause::CertUnverified;
}

using Answer = std::function<void(bool accepted)>;

/* Raises the shared warning for a fallback to plainUrl. message is translated
 * and has %1 for the URL. answer is called once, on the UI thread: true for
 * the countdown or "Accept anyway" (restart over plain RTMP), false for
 * Cancel or a closed dialog. */
inline void Warn(const QString &title, const QString &message, const std::string &plainUrl, Cause cause, Answer answer)
{
	std::string id = Id(plainUrl, cause);
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
