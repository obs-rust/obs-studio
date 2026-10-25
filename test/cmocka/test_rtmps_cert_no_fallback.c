/* An RTMPS stream whose server certificate does not verify must not fall
 * back to plain RTMP (obs-rust/obs-studio#2205, docs/fallback-warning.md
 * rule 7). The frontend retries a failed RTMPS start over RTMP only for a
 * connection failure: timeout, refusal, reset, or a handshake that fails for
 * another reason. A certificate that does not verify, also one issued for
 * another host name, stops the start; the RTMP output reports it through its
 * "rtmp_tls_cert_unverified" proc.
 *
 * The test builds the real rtmp-stream.c and librtmp into this executable and
 * connects to a TLS listener on the loopback interface. The listener presents
 * a certificate the test makes at run time, issued for another host. With
 * that certificate trusted, only the host name is wrong; without it, the
 * certificate is also untrusted. A listener that closes the connection during
 * the handshake, and a port with no listener, are connection failures that
 * still fall back. The output is a small fake, so no display or encoder is
 * needed. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <arpa/inet.h>
#include <netinet/in.h>
#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

#include <mbedtls/ctr_drbg.h>
#include <mbedtls/ecp.h>
#include <mbedtls/entropy.h>
#include <mbedtls/net_sockets.h>
#include <mbedtls/pk.h>
#include <mbedtls/ssl.h>
#include <mbedtls/x509_crt.h>

#include "librtmp/rtmp.h"

/* Route the libobs calls rtmp-stream.c makes on its output to the fakes
 * below, and RTMP_Connect through a wrapper that can add a trusted
 * certificate. */
#define obs_output_get_name fake_output_get_name
#define obs_output_set_last_error fake_output_set_last_error
#define obs_output_end_data_capture fake_output_end_data_capture
#define obs_output_get_proc_handler fake_output_get_proc_handler
static int test_rtmp_connect(RTMP *r, RTMPPacket *cp);
#define RTMP_Connect test_rtmp_connect

#include "rtmp-stream.c"

#undef RTMP_Connect

const char *obs_module_text(const char *lookup)
{
	return lookup;
}

/* ------------------------------------------------------------------------- */
/* fakes */

static int fake_output; /* only its address is used */
static proc_handler_t *output_procs;

const char *fake_output_get_name(const obs_output_t *output)
{
	(void)output;
	return "cert test";
}

void fake_output_set_last_error(obs_output_t *output, const char *message)
{
	(void)output;
	(void)message;
}

void fake_output_end_data_capture(obs_output_t *output)
{
	(void)output;
}

proc_handler_t *fake_output_get_proc_handler(const obs_output_t *output)
{
	(void)output;
	return output_procs;
}

/* A certificate the client trusts in addition to the system's, or NULL. */
static mbedtls_x509_crt *trusted_extra;

static int test_rtmp_connect(RTMP *r, RTMPPacket *cp)
{
	if (trusted_extra && r->RTMP_TLS_ctx)
		mbedtls_ssl_conf_ca_chain(&r->RTMP_TLS_ctx->conf, trusted_extra, NULL);
	return RTMP_Connect(r, cp);
}

/* ------------------------------------------------------------------------- */
/* the server's certificate */

static mbedtls_entropy_context entropy;
static mbedtls_ctr_drbg_context drbg;
static mbedtls_pk_context server_key;
static mbedtls_x509_crt server_cert;

/* A self-signed certificate for host, with its key. */
static void make_cert(const char *host)
{
	char name[128];
	unsigned char pem[4096];
	unsigned char serial[] = {0x01};
	mbedtls_x509write_cert w;

	mbedtls_pk_init(&server_key);
	assert_int_equal(mbedtls_pk_setup(&server_key, mbedtls_pk_info_from_type(MBEDTLS_PK_ECKEY)), 0);
	assert_int_equal(mbedtls_ecp_gen_key(MBEDTLS_ECP_DP_SECP256R1, mbedtls_pk_ec(server_key),
					     mbedtls_ctr_drbg_random, &drbg),
			 0);

	snprintf(name, sizeof(name), "CN=%s", host);
	mbedtls_x509write_crt_init(&w);
	mbedtls_x509write_crt_set_version(&w, MBEDTLS_X509_CRT_VERSION_3);
	mbedtls_x509write_crt_set_md_alg(&w, MBEDTLS_MD_SHA256);
	mbedtls_x509write_crt_set_subject_key(&w, &server_key);
	mbedtls_x509write_crt_set_issuer_key(&w, &server_key);
	assert_int_equal(mbedtls_x509write_crt_set_subject_name(&w, name), 0);
	assert_int_equal(mbedtls_x509write_crt_set_issuer_name(&w, name), 0);
	assert_int_equal(mbedtls_x509write_crt_set_serial_raw(&w, serial, sizeof(serial)), 0);
	assert_int_equal(mbedtls_x509write_crt_set_validity(&w, "20200101000000", "20991231235959"), 0);
	assert_int_equal(mbedtls_x509write_crt_set_basic_constraints(&w, 1, -1), 0);
	assert_int_equal(mbedtls_x509write_crt_pem(&w, pem, sizeof(pem), mbedtls_ctr_drbg_random, &drbg), 0);
	mbedtls_x509write_crt_free(&w);

	mbedtls_x509_crt_init(&server_cert);
	assert_int_equal(mbedtls_x509_crt_parse(&server_cert, pem, strlen((char *)pem) + 1), 0);
}

/* ------------------------------------------------------------------------- */
/* the listener */

enum listener {
	LISTENER_TLS,   /* completes TCP and presents server_cert */
	LISTENER_RESET, /* completes TCP and closes during the handshake */
};

struct server {
	enum listener kind;
	int fd;
	int port;
	pthread_t thread;
};

static void *serve(void *param)
{
	struct server *s = param;
	int conn = accept(s->fd, NULL, NULL);
	if (conn < 0)
		return NULL;

	if (s->kind == LISTENER_TLS) {
		mbedtls_ssl_config conf;
		mbedtls_ssl_context ssl;
		mbedtls_net_context net = {.fd = conn};

		mbedtls_ssl_config_init(&conf);
		mbedtls_ssl_init(&ssl);
		mbedtls_ssl_config_defaults(&conf, MBEDTLS_SSL_IS_SERVER, MBEDTLS_SSL_TRANSPORT_STREAM,
					    MBEDTLS_SSL_PRESET_DEFAULT);
		mbedtls_ssl_conf_max_tls_version(&conf, MBEDTLS_SSL_VERSION_TLS1_2);
		mbedtls_ssl_conf_rng(&conf, mbedtls_ctr_drbg_random, &drbg);
		mbedtls_ssl_conf_own_cert(&conf, &server_cert, &server_key);
		if (mbedtls_ssl_setup(&ssl, &conf) == 0) {
			mbedtls_ssl_set_bio(&ssl, &net, mbedtls_net_send, mbedtls_net_recv, NULL);
			/* Fails once the client rejects the certificate. */
			mbedtls_ssl_handshake(&ssl);
		}
		mbedtls_ssl_free(&ssl);
		mbedtls_ssl_config_free(&conf);
	} else {
		char hello[16];
		(void)recv(conn, hello, sizeof(hello), 0);
	}
	close(conn);
	return NULL;
}

static void start_server(struct server *s, enum listener kind)
{
	struct sockaddr_in addr = {.sin_family = AF_INET, .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
	socklen_t len = sizeof(addr);

	s->kind = kind;
	s->fd = socket(AF_INET, SOCK_STREAM, 0);
	assert_true(s->fd >= 0);
	assert_int_equal(bind(s->fd, (struct sockaddr *)&addr, sizeof(addr)), 0);
	assert_int_equal(listen(s->fd, 1), 0);
	assert_int_equal(getsockname(s->fd, (struct sockaddr *)&addr, &len), 0);
	s->port = ntohs(addr.sin_port);
	assert_int_equal(pthread_create(&s->thread, NULL, serve, s), 0);
}

static void stop_server(struct server *s)
{
	shutdown(s->fd, SHUT_RDWR);
	pthread_join(s->thread, NULL);
	close(s->fd);
}

/* A port on the loopback interface with nothing listening. */
static int closed_port(void)
{
	struct sockaddr_in addr = {.sin_family = AF_INET, .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
	socklen_t len = sizeof(addr);
	int fd = socket(AF_INET, SOCK_STREAM, 0);
	assert_true(fd >= 0);
	assert_int_equal(bind(fd, (struct sockaddr *)&addr, sizeof(addr)), 0);
	assert_int_equal(getsockname(fd, (struct sockaddr *)&addr, &len), 0);
	close(fd);
	return ntohs(addr.sin_port);
}

/* ------------------------------------------------------------------------- */
/* connecting */

struct result {
	int code;
	bool proc_found;
	bool unverified;
};

/* Connects the RTMP output to rtmps://127.0.0.1:port/live and asks it, as
 * the frontend does, whether the certificate failed to verify. */
static struct result connect_to(int port)
{
	struct result r = {0};
	calldata_t cd = {0};

	output_procs = proc_handler_create();
	struct rtmp_stream *stream = rtmp_stream_create(NULL, (obs_output_t *)&fake_output);
	assert_non_null(stream);
	dstr_printf(&stream->path, "rtmps://127.0.0.1:%d/live", port);
	dstr_copy(&stream->key, "key");

	r.code = try_connect(stream);

	r.proc_found = proc_handler_call(output_procs, "rtmp_tls_cert_unverified", &cd);
	r.unverified = r.proc_found && calldata_bool(&cd, "unverified");
	calldata_free(&cd);

	rtmp_stream_destroy(stream);
	proc_handler_destroy(output_procs);
	output_procs = NULL;
	return r;
}

/* The certificate is trusted but issued for another host: the host name does
 * not match, so the start stops and does not fall back. */
static void cert_for_other_host_does_not_fall_back(void **state)
{
	(void)state;
	struct server s;
	trusted_extra = &server_cert;
	start_server(&s, LISTENER_TLS);

	struct result r = connect_to(s.port);

	stop_server(&s);
	trusted_extra = NULL;
	assert_int_equal(r.code, OBS_OUTPUT_CONNECT_FAILED);
	assert_true(r.proc_found);
	assert_true(r.unverified);
}

/* The certificate is not trusted (and for another host): no fallback. */
static void untrusted_cert_does_not_fall_back(void **state)
{
	(void)state;
	struct server s;
	start_server(&s, LISTENER_TLS);

	struct result r = connect_to(s.port);

	stop_server(&s);
	assert_int_equal(r.code, OBS_OUTPUT_CONNECT_FAILED);
	assert_true(r.proc_found);
	assert_true(r.unverified);
}

/* The server closes the connection during the handshake: a connection
 * failure, so the fallback stays open. */
static void reset_handshake_still_falls_back(void **state)
{
	(void)state;
	struct server s;
	start_server(&s, LISTENER_RESET);

	struct result r = connect_to(s.port);

	stop_server(&s);
	assert_int_equal(r.code, OBS_OUTPUT_CONNECT_FAILED);
	assert_true(r.proc_found);
	assert_false(r.unverified);
}

/* Nothing listens: a refused connection, so the fallback stays open. */
static void refused_connection_still_falls_back(void **state)
{
	(void)state;
	struct result r = connect_to(closed_port());

	assert_int_equal(r.code, OBS_OUTPUT_CONNECT_FAILED);
	assert_true(r.proc_found);
	assert_false(r.unverified);
}

static int setup(void **state)
{
	(void)state;
	const char *pers = "rtmps-cert-test";
	mbedtls_entropy_init(&entropy);
	mbedtls_ctr_drbg_init(&drbg);
	assert_int_equal(mbedtls_ctr_drbg_seed(&drbg, mbedtls_entropy_func, &entropy, (const unsigned char *)pers,
					       strlen(pers)),
			 0);
	make_cert("other.example");
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	mbedtls_x509_crt_free(&server_cert);
	mbedtls_pk_free(&server_key);
	mbedtls_ctr_drbg_free(&drbg);
	mbedtls_entropy_free(&entropy);
	return 0;
}

int main(void)
{
	signal(SIGPIPE, SIG_IGN);

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(cert_for_other_host_does_not_fall_back),
		cmocka_unit_test(untrusted_cert_does_not_fall_back),
		cmocka_unit_test(reset_handshake_still_falls_back),
		cmocka_unit_test(refused_connection_still_falls_back),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
