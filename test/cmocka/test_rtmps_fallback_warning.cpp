/* The RTMPS fallback (obs-rust/obs-studio#2205) through the shared fallback
 * warning (obs_frontend_warn_fallback, docs/fallback-warning.md). When a
 * stream that rtmp-services upgraded to RTMPS fails to connect, the frontend
 * raises the warning with Cancel, its standard wording and the plain RTMP URL,
 * and restarts over RTMP only when the answer is to continue. A certificate
 * that did not verify never falls back; a system without trusted root
 * certificates falls back with a message that says so. obs_frontend_warn_fallback is a fake
 * here that records its arguments. */

#include <fstream>
#include <sstream>
#include <string>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
extern "C" {
#include <cmocka.h>
}

#include "RtmpsFallback.hpp"

static struct {
	int calls;
	std::string id;
	std::string title;
	std::string message;
	bool can_cancel;
	obs_frontend_fallback_cb cb;
	void *param;
} raised;

extern "C" void obs_frontend_warn_fallback(const char *id, const char *title, const char *message, bool can_cancel,
					   obs_frontend_fallback_cb cb, void *param)
{
	raised.calls++;
	raised.id = id;
	raised.title = title;
	raised.message = message;
	raised.can_cancel = can_cancel;
	raised.cb = cb;
	raised.param = param;
}

static int reset(void **state)
{
	(void)state;
	raised = {};
	return 0;
}

static const char *url = "rtmp://ingest.example/app";

/* The warning has Cancel, the fallback's id, the title, and the message with
 * the plain URL in it. Nothing is answered before the warning answers. */
static void raises_shared_warning_with_cancel(void **state)
{
	(void)state;
	int answers = 0;
	RtmpsFallback::Warn("Streaming unencrypted (RTMP)", "OBS streams to %1 unencrypted.", url,
			    RtmpsFallback::Cause::Connection, [&](bool) { answers++; });

	assert_int_equal(raised.calls, 1);
	assert_true(raised.can_cancel);
	assert_string_equal(raised.id.c_str(), "rtmp.unencrypted:rtmp://ingest.example/app");
	assert_string_equal(raised.title.c_str(), "Streaming unencrypted (RTMP)");
	assert_string_equal(raised.message.c_str(), "OBS streams to rtmp://ingest.example/app unencrypted.");
	assert_non_null(raised.cb);
	assert_int_equal(answers, 0);

	raised.cb(raised.param, true);
	assert_int_equal(answers, 1);
}

/* "Accept anyway" or the countdown: the answer is to continue over RTMP. */
static void accept_passes_true(void **state)
{
	(void)state;
	int answer = -1;
	RtmpsFallback::Warn("T", "M %1", url, RtmpsFallback::Cause::Connection,
			    [&](bool accepted) { answer = accepted; });
	raised.cb(raised.param, true);
	assert_int_equal(answer, 1);
}

/* Cancel or a closed dialog: the start stops. */
static void cancel_passes_false(void **state)
{
	(void)state;
	int answer = -1;
	RtmpsFallback::Warn("T", "M %1", url, RtmpsFallback::Cause::Connection,
			    [&](bool accepted) { answer = accepted; });
	raised.cb(raised.param, false);
	assert_int_equal(answer, 0);
}

/* Each server has its own id, so each server's first failure in a session
 * shows the dialog. */
static void id_names_the_server(void **state)
{
	(void)state;
	if (RtmpsFallback::IdPerServer) {
		assert_string_not_equal(RtmpsFallback::Id("rtmp://a.example/app").c_str(),
					RtmpsFallback::Id("rtmp://b.example/app").c_str());
	} else {
		assert_string_equal(RtmpsFallback::Id("rtmp://a.example/app").c_str(), "rtmp.unencrypted");
	}
	assert_string_equal(RtmpsFallback::Id("").c_str(), "rtmp.unencrypted");
	assert_int_equal(strncmp(RtmpsFallback::Id(url).c_str(), "rtmp.unencrypted", 16), 0);
}

/* Only a connection failure of a service stream falls back; a certificate
 * that did not verify, multitrack video and other stop codes do not. */
static void falls_back_only_on_connection_failure(void **state)
{
	(void)state;
	using RtmpsFallback::Cause;
	assert_true(RtmpsFallback::Allowed(OBS_OUTPUT_CONNECT_FAILED, false, Cause::Connection));
	assert_true(RtmpsFallback::Allowed(OBS_OUTPUT_CONNECT_FAILED, false, Cause::NoRootCerts));
	assert_false(RtmpsFallback::Allowed(OBS_OUTPUT_CONNECT_FAILED, false, Cause::CertUnverified));
	assert_false(RtmpsFallback::Allowed(OBS_OUTPUT_CONNECT_FAILED, true, Cause::Connection));
	assert_false(RtmpsFallback::Allowed(OBS_OUTPUT_BAD_PATH, false, Cause::Connection));
	assert_false(RtmpsFallback::Allowed(OBS_OUTPUT_INVALID_STREAM, false, Cause::Connection));
	assert_false(RtmpsFallback::Allowed(OBS_OUTPUT_DISCONNECTED, false, Cause::Connection));
	assert_false(RtmpsFallback::Allowed(OBS_OUTPUT_SUCCESS, false, Cause::Connection));
}

/* The RTMP output's rtmp_tls_failure strings map to the causes; anything
 * else, or no answer, is an ordinary connection failure. */
static void parses_the_outputs_cause(void **state)
{
	(void)state;
	using RtmpsFallback::Cause;
	assert_true(RtmpsFallback::ParseCause("cert_unverified") == Cause::CertUnverified);
	assert_true(RtmpsFallback::ParseCause("no_root_certs") == Cause::NoRootCerts);
	assert_true(RtmpsFallback::ParseCause("") == Cause::Connection);
	assert_true(RtmpsFallback::ParseCause("other") == Cause::Connection);
	assert_true(RtmpsFallback::ParseCause(nullptr) == Cause::Connection);
}

/* No trusted root certificates: the fallback is offered with its own id and
 * the message that names the cause. Other failures keep the normal message. */
static void no_root_certs_warning_says_why(void **state)
{
	(void)state;
	using RtmpsFallback::Cause;
	assert_string_equal(RtmpsFallback::MessageKey(Cause::NoRootCerts), "Output.RtmpsFallback.NoRootCerts");
	assert_string_equal(RtmpsFallback::MessageKey(Cause::Connection), "Output.RtmpsFallback.Message");

	int answer = -1;
	RtmpsFallback::Warn("T", "No certificates; streaming to %1.", url, Cause::NoRootCerts,
			    [&](bool accepted) { answer = accepted; });
	assert_int_equal(raised.calls, 1);
	assert_true(raised.can_cancel);
	assert_string_equal(raised.id.c_str(), "rtmp.unencrypted.nocerts:rtmp://ingest.example/app");
	assert_string_equal(raised.message.c_str(), "No certificates; streaming to rtmp://ingest.example/app.");
	raised.cb(raised.param, true);
	assert_int_equal(answer, 1);

	assert_string_not_equal(RtmpsFallback::Id(url, Cause::NoRootCerts).c_str(),
				RtmpsFallback::Id(url, Cause::Connection).c_str());
}

static std::string locale_value(const std::string &text, const char *key)
{
	std::istringstream in(text);
	std::string line, prefix = std::string(key) + "=\"";
	while (std::getline(in, line)) {
		if (line.compare(0, prefix.size(), prefix) == 0 && line.size() > prefix.size()) {
			return line.substr(prefix.size(), line.size() - prefix.size() - 1);
		}
	}
	return "";
}

/* The strings follow docs/fallback-warning.md: the title "Streaming
 * unencrypted (RTMP)", the term "unencrypted", and a message that names the
 * URL. The buttons come from the shared warning, so the fallback has none. */
static void wording_follows_the_standard(void **state)
{
	(void)state;
	std::ifstream f(FRONTEND_LOCALE);
	assert_true(f.good());
	std::stringstream ss;
	ss << f.rdbuf();
	std::string text = ss.str();

	assert_string_equal(locale_value(text, "Output.RtmpsFallback.Title").c_str(), "Streaming unencrypted (RTMP)");
	std::string message = locale_value(text, "Output.RtmpsFallback.Message");
	assert_non_null(strstr(message.c_str(), "unencrypted"));
	assert_non_null(strstr(message.c_str(), "%1"));
	assert_null(strstr(message.c_str(), "insecure"));
	assert_null(strstr(message.c_str(), "unsafe"));
	assert_string_equal(locale_value(text, "Output.RtmpsFallback.Proceed").c_str(), "");

	/* No root certificates: what failed (the trusted certificates), what OBS
	 * does instead (unencrypted over RTMP), and what the user can do. */
	std::string nocerts = locale_value(text, "Output.RtmpsFallback.NoRootCerts");
	assert_false(nocerts.empty());
	assert_true(nocerts != message);
	assert_non_null(strstr(nocerts.c_str(), "trusted certificates"));
	assert_non_null(strstr(nocerts.c_str(), "unencrypted"));
	assert_non_null(strstr(nocerts.c_str(), "CA certificates package"));
	assert_non_null(strstr(nocerts.c_str(), "Cancel"));
	assert_non_null(strstr(nocerts.c_str(), "%1"));
	const char *what = strstr(nocerts.c_str(), "trusted certificates");
	const char *instead = strstr(nocerts.c_str(), "unencrypted");
	const char *todo = strstr(nocerts.c_str(), "CA certificates package");
	assert_true(what < instead && instead < todo);
	assert_string_equal(locale_value(text, "FallbackWarning.AcceptAnyway").c_str(), "Accept anyway");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup(raises_shared_warning_with_cancel, reset),
		cmocka_unit_test_setup(accept_passes_true, reset),
		cmocka_unit_test_setup(cancel_passes_false, reset),
		cmocka_unit_test_setup(id_names_the_server, reset),
		cmocka_unit_test_setup(falls_back_only_on_connection_failure, reset),
		cmocka_unit_test_setup(parses_the_outputs_cause, reset),
		cmocka_unit_test_setup(no_root_certs_warning_says_why, reset),
		cmocka_unit_test_setup(wording_follows_the_standard, reset),
	};
	return cmocka_run_group_tests(tests, nullptr, nullptr);
}
