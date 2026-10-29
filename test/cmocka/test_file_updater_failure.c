/* obs-rust/obs-studio#1347: a single-file update whose HTTP request fails
 * must tell its user the attempt ended, so the rtmp-services ingest refresh
 * wait (service-ingest, dacast) ends at once instead of at its 3-5 s
 * timeout. A successful download still reaches the confirm callback with
 * the same data as before. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <string.h>
#include <unistd.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <sys/socket.h>

#include <util/platform.h>
#include <util/threading.h>
#include <util/dstr.h>
#include <util/bmem.h>

#include <file-updater/file-updater.h>
#include "shared/file-updater/file-updater/curl-accept-encoding.h"
#include "plugins/rtmp-services/service-specific/ingest-refresh-wait.h"

#define TIMEOUT_MS 4000UL
#define BODY "{\"ingests\":[{\"name\":\"local\",\"url_template\":\"rtmp://127.0.0.1/app\"}]}"

/* The Accept-Encoding header of the request the server actually received,
 * kept so the assertions can read what curl advertised on the wire. */
static char accept_encoding[256];

/* Whether value lists token as a comma-separated coding, per RFC 9110:
 * "br" must not match inside "brotli-thing" or at the end of another word. */
static bool lists_coding(const char *value, const char *token)
{
	size_t n = strlen(token);
	const char *p = value;

	while (*p) {
		const char *end = strchr(p, ',');
		size_t len = end ? (size_t)(end - p) : strlen(p);

		while (len && (*p == ' ' || *p == '\t')) {
			p++;
			len--;
		}
		while (len && (p[len - 1] == ' ' || p[len - 1] == '\t'))
			len--;
		if (len == n && strncmp(p, token, n) == 0)
			return true;
		if (!end)
			break;
		p = end + 1;
	}
	return false;
}

struct user {
	struct ingest_refresh_wait wait;
	int confirm_calls;
	int done_calls;
	bool done_success;
	char *data;
};

/* Mirrors service-ingest's ingest_update: the confirm callback signals the
 * wait when data arrives. */
static bool confirm_cb(void *param, struct file_download_data *file)
{
	struct user *u = param;
	u->confirm_calls++;
	bfree(u->data);
	u->data = bstrdup((const char *)file->buffer.array);
	ingest_refresh_wait_complete(&u->wait, true);
	return true;
}

static void done_cb(void *param, bool success)
{
	struct user *u = param;
	u->done_calls++;
	u->done_success = success;
	ingest_refresh_wait_complete(&u->wait, success);
}

static int listen_socket(unsigned short *port)
{
	int fd = socket(AF_INET, SOCK_STREAM, 0);
	assert_true(fd >= 0);
	struct sockaddr_in addr = {0};
	addr.sin_family = AF_INET;
	addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
	assert_int_equal(bind(fd, (struct sockaddr *)&addr, sizeof(addr)), 0);
	socklen_t len = sizeof(addr);
	assert_int_equal(getsockname(fd, (struct sockaddr *)&addr, &len), 0);
	*port = ntohs(addr.sin_port);
	return fd;
}

static uint64_t run_refresh(struct user *u, unsigned short port, bool *ended)
{
	struct dstr url = {0};
	dstr_printf(&url, "http://127.0.0.1:%u/ingests.json", (unsigned)port);

	ingest_refresh_wait_init(&u->wait);
	ingest_refresh_wait_reset(&u->wait);

	uint64_t start = os_gettime_ns();
	update_info_t *info =
		update_info_create_single_with_done("[test] ", "obs-test", url.array, confirm_cb, done_cb, u);
	assert_non_null(info);
	*ended = ingest_refresh_wait_for(&u->wait, TIMEOUT_MS);
	uint64_t elapsed_ms = (os_gettime_ns() - start) / 1000000;

	update_info_destroy(info);
	ingest_refresh_wait_free(&u->wait);
	dstr_free(&url);
	return elapsed_ms;
}

static void failed_download_ends_wait(void **state)
{
	(void)state;
	unsigned short port;
	/* Bound then closed: nothing listens, the connect is refused. */
	close(listen_socket(&port));

	struct user u = {0};
	bool ended;
	uint64_t ms = run_refresh(&u, port, &ended);

	assert_true(ended);
	assert_true(ms < TIMEOUT_MS / 2);
	assert_int_equal(u.confirm_calls, 0);
	assert_int_equal(u.done_calls, 1);
	assert_false(u.done_success);
	assert_null(u.data);
}

static void *serve_once(void *data)
{
	int fd = (int)(intptr_t)data;
	int client = accept(fd, NULL, NULL);
	if (client < 0)
		return NULL;

	char req[4096];
	size_t got = 0;
	while (got < sizeof(req) - 1) {
		ssize_t n = recv(client, req + got, sizeof(req) - 1 - got, 0);
		if (n <= 0)
			break;
		got += (size_t)n;
		req[got] = 0;
		if (strstr(req, "\r\n\r\n"))
			break;
	}
	req[got] = 0;

	/* Keep what the client asked to accept, for the encoding assertions. */
	accept_encoding[0] = 0;
	{
		const char *hdr = strstr(req, "\r\nAccept-Encoding:");

		if (hdr) {
			hdr += strlen("\r\nAccept-Encoding:");
			while (*hdr == ' ' || *hdr == '	')
				hdr++;
			size_t i = 0;
			while (*hdr && *hdr != '\r' && i < sizeof(accept_encoding) - 1)
				accept_encoding[i++] = *hdr++;
			accept_encoding[i] = 0;
		}
	}

	struct dstr resp = {0};
	dstr_printf(&resp,
		    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: %zu\r\n"
		    "Connection: close\r\n\r\n%s",
		    strlen(BODY), BODY);
	size_t sent = 0;
	while (sent < resp.len) {
		ssize_t n = send(client, resp.array + sent, resp.len - sent, 0);
		if (n <= 0)
			break;
		sent += (size_t)n;
	}
	dstr_free(&resp);
	close(client);
	return NULL;
}

static void successful_download_reaches_confirm(void **state)
{
	(void)state;
	unsigned short port;
	int fd = listen_socket(&port);
	assert_int_equal(listen(fd, 1), 0);
	pthread_t server;
	assert_int_equal(pthread_create(&server, NULL, serve_once, (void *)(intptr_t)fd), 0);

	struct user u = {0};
	bool ended;
	run_refresh(&u, port, &ended);
	pthread_join(server, NULL);
	close(fd);

	assert_true(ended);
	assert_int_equal(u.confirm_calls, 1);
	assert_non_null(u.data);
	assert_string_equal(u.data, BODY);
	/* What curl advertised on the wire. With "" the calls ask for every
	 * coding this libcurl decodes, so br comes back where the linked Brotli
	 * is one this repo pins (#2211). Apple keeps the stop-gap: the OS
	 * libcurl's Brotli is not pinned here, so br must not be advertised
	 * there, and gzip only where that libcurl has zlib (#2453). */
	const curl_version_info_data *vi = curl_version_info(CURLVERSION_NOW);
#if OBS_CURL_ACCEPT_ENCODING_UNPINNED_BROTLI
	if (vi->features & CURL_VERSION_LIBZ)
		assert_non_null(strstr(accept_encoding, "gzip"));
	assert_false(lists_coding(accept_encoding, "br"));
#else
	if (vi->features & CURL_VERSION_BROTLI)
		assert_true(lists_coding(accept_encoding, "br"));
	else
		assert_false(lists_coding(accept_encoding, "br"));
#endif
	/* update_info_destroy joined the thread: the done callback has run. */
	assert_int_equal(u.done_calls, 1);
	assert_true(u.done_success);
	bfree(u.data);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(failed_download_ends_wait),
		cmocka_unit_test(successful_download_reaches_confirm),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
