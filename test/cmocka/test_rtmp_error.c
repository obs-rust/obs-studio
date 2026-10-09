#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <cmocka.h>

#include <librtmp/rtmp.h>
#include <librtmp/log.h>

static char last_error[256];

static void capture_log(int level, const char *format, va_list args)
{
	if (level <= RTMP_LOGERROR)
		vsnprintf(last_error, sizeof(last_error), format, args);
}

static int setup(void **state)
{
#ifdef _WIN32
	WSADATA data;
	assert_int_equal(WSAStartup(MAKEWORD(2, 2), &data), 0);
#endif
	RTMP *rtmp = RTMP_Alloc();
	assert_non_null(rtmp);
	RTMP_Init(rtmp);
	RTMP_EnableWrite(rtmp);
	/* An owned, unconnected socket lets us verify error cleanup. No server,
	 * DNS lookup, TLS handshake, or network traffic is needed. */
	rtmp->m_sb.sb_socket = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
	assert_true(RTMP_IsConnected(rtmp));
	rtmp->m_methodCalls = calloc(1, sizeof(*rtmp->m_methodCalls));
	assert_non_null(rtmp->m_methodCalls);
	rtmp->m_numCalls = 1;
	rtmp->m_methodCalls[0].num = 1;
	rtmp->m_methodCalls[0].name.av_val = malloc(sizeof("connect"));
	assert_non_null(rtmp->m_methodCalls[0].name.av_val);
	memcpy(rtmp->m_methodCalls[0].name.av_val, "connect", sizeof("connect"));
	rtmp->m_methodCalls[0].name.av_len = sizeof("connect") - 1;
	last_error[0] = '\0';
	RTMP_LogSetCallback(capture_log);
	*state = rtmp;
	return 0;
}

static int teardown(void **state)
{
	RTMP *rtmp = *state;
	RTMP_Close(rtmp);
	RTMP_Free(rtmp);
#ifdef _WIN32
	WSACleanup();
#endif
	return 0;
}

/* Construct a complete AMF0 _error reply. AMF_INVALID omits description;
 * AMF_NULL/AMF_NUMBER exercise AMFProp_GetString's non-string result. */
static void receive_error(RTMP *rtmp, AMFDataType type, const char *description, bool include_object, double txn)
{
	char body[512] = {0};
	char *end = body + sizeof(body);
	AVal method = AVC("_error");
	AVal name = AVC("description");
	char *p = AMF_EncodeString(body, end, &method);
	assert_non_null(p);
	p = AMF_EncodeNumber(p, end, txn);
	assert_non_null(p);
	*p++ = AMF_NULL;
	if (include_object) {
		*p++ = AMF_OBJECT;
		if (type == AMF_STRING) {
			AVal value = {(char *)description, (int)strlen(description)};
			p = AMF_EncodeNamedString(p, end, &name, &value);
			assert_non_null(p);
		} else if (type != AMF_INVALID) {
			p = AMF_EncodeInt16(p, end, name.av_len);
			assert_non_null(p);
			memcpy(p, name.av_val, name.av_len);
			p += name.av_len;
			if (type == AMF_NUMBER) {
				p = AMF_EncodeNumber(p, end, 42);
				assert_non_null(p);
			} else {
				*p++ = (char)type;
			}
		}
		*p++ = 0;
		*p++ = 0;
		*p++ = AMF_OBJECT_END;
	}
	RTMPPacket packet = {0};
	packet.m_packetType = RTMP_PACKET_TYPE_INVOKE;
	packet.m_body = body;
	packet.m_nBodySize = (uint32_t)(p - body);
	assert_int_equal(RTMP_ClientPacket(rtmp, &packet), 0);
}

static void assert_connection_rejected(RTMP *rtmp)
{
	assert_false(RTMP_IsConnected(rtmp));
	assert_false(rtmp->m_bPlaying);
	assert_int_equal(rtmp->m_numCalls, 0);
	assert_true(rtmp->Link.pFlags & RTMP_PUB_CLEAN);
	assert_true(last_error[0] != '\0');
}

static void missing_description(void **state)
{
	RTMP *rtmp = *state;
	receive_error(rtmp, AMF_INVALID, NULL, true, 1);
	assert_connection_rejected(rtmp);
}

static void empty_description(void **state)
{
	RTMP *rtmp = *state;
	receive_error(rtmp, AMF_STRING, "", true, 1);
	assert_connection_rejected(rtmp);
}

static void null_description(void **state)
{
	RTMP *rtmp = *state;
	receive_error(rtmp, AMF_NULL, NULL, true, 1);
	assert_connection_rejected(rtmp);
}

static void numeric_description(void **state)
{
	RTMP *rtmp = *state;
	receive_error(rtmp, AMF_NUMBER, NULL, true, 1);
	assert_connection_rejected(rtmp);
}

static void missing_error_object(void **state)
{
	RTMP *rtmp = *state;
	receive_error(rtmp, AMF_INVALID, NULL, false, 1);
	assert_connection_rejected(rtmp);
}

static void ordinary_description_is_not_authentication(void **state)
{
	RTMP *rtmp = *state;
	receive_error(rtmp, AMF_STRING, "Connection rejected", true, 1);
	assert_int_equal(rtmp->m_numCalls, 0);
	assert_int_equal(rtmp->Link.pFlags, 0);
	assert_true(RTMP_IsConnected(rtmp));
}

static void adobe_authentication_error_still_recognized(void **state)
{
	RTMP *rtmp = *state;
	receive_error(rtmp, AMF_STRING, "authmod=adobe?reason=authfailed", true, 1);
	assert_true(rtmp->Link.pFlags & RTMP_PUB_CLEAN);
	assert_non_null(strstr(last_error, "Authentication failed"));
	assert_int_equal(rtmp->m_numCalls, 0);
}

static void llnw_authentication_error_still_recognized(void **state)
{
	RTMP *rtmp = *state;
	receive_error(rtmp, AMF_STRING, "authmod=llnw?reason=authfail", true, 1);
	assert_true(rtmp->Link.pFlags & RTMP_PUB_CLEAN);
	assert_non_null(strstr(last_error, "Authentication failed"));
	assert_int_equal(rtmp->m_numCalls, 0);
}

static void unmatched_transaction_is_ignored(void **state)
{
	RTMP *rtmp = *state;
	receive_error(rtmp, AMF_INVALID, NULL, true, 2);
	assert_int_equal(rtmp->m_numCalls, 1);
	assert_int_equal(rtmp->Link.pFlags, 0);
	assert_true(RTMP_IsConnected(rtmp));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(missing_description, setup, teardown),
		cmocka_unit_test_setup_teardown(empty_description, setup, teardown),
		cmocka_unit_test_setup_teardown(null_description, setup, teardown),
		cmocka_unit_test_setup_teardown(numeric_description, setup, teardown),
		cmocka_unit_test_setup_teardown(missing_error_object, setup, teardown),
		cmocka_unit_test_setup_teardown(ordinary_description_is_not_authentication, setup, teardown),
		cmocka_unit_test_setup_teardown(adobe_authentication_error_still_recognized, setup, teardown),
		cmocka_unit_test_setup_teardown(llnw_authentication_error_still_recognized, setup, teardown),
		cmocka_unit_test_setup_teardown(unmatched_transaction_is_ignored, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
