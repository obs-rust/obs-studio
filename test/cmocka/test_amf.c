#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <cmocka.h>

#include "librtmp/amf.h"

/* librtmp's AMF decoder on nested objects and arrays, as an RTMP server
 * sends them in command and metadata messages. Each nesting level is a
 * recursion, so the decoder fails past AMF_MAX_DEPTH (128) levels instead of
 * running out of stack on a deeply nested message. */

#define MAX_DEPTH 128
#define DEEP 200000

struct buf {
	char *data;
	size_t len;
	size_t cap;
};

static void put(struct buf *b, const char *bytes, size_t len)
{
	if (b->len + len > b->cap) {
		b->cap = (b->cap + len) * 2;
		b->data = realloc(b->data, b->cap);
		assert_non_null(b->data);
	}
	memcpy(b->data + b->len, bytes, len);
	b->len += len;
}

#define PUT(b, ...)                                           \
	do {                                                  \
		const char bytes_[] = {__VA_ARGS__};          \
		put((b), bytes_, sizeof(bytes_));             \
	} while (0)

/* AMF0: `depth` objects, each holding the next as property "a" */
static struct buf nested_objects(size_t depth)
{
	struct buf b = {0};
	PUT(&b, AMF_OBJECT);
	for (size_t i = 1; i < depth; i++)
		PUT(&b, 0, 1, 'a', AMF_OBJECT);
	for (size_t i = 0; i < depth; i++)
		PUT(&b, 0, 0, AMF_OBJECT_END);
	return b;
}

/* AMF0: `depth` strict arrays of one element, the innermost holding a
 * boolean */
static struct buf nested_arrays(size_t depth)
{
	struct buf b = {0};
	for (size_t i = 0; i < depth; i++)
		PUT(&b, AMF_STRICT_ARRAY, 0, 0, 0, 1);
	PUT(&b, AMF_BOOLEAN, 1);
	return b;
}

/* AMF0 ECMA arrays (count, then named properties up to an end marker) */
static struct buf nested_ecma_arrays(size_t depth)
{
	struct buf b = {0};
	PUT(&b, AMF_ECMA_ARRAY, 0, 0, 0, 1);
	for (size_t i = 1; i < depth; i++)
		PUT(&b, 0, 1, 'a', AMF_ECMA_ARRAY, 0, 0, 0, 1);
	for (size_t i = 0; i < depth; i++)
		PUT(&b, 0, 0, AMF_OBJECT_END);
	return b;
}

/* AMF3 inside AMF0: dynamic objects, each holding the next as "a". */
static struct buf nested_amf3_objects(size_t depth)
{
	struct buf b = {0};
	PUT(&b, AMF_AVMPLUS);
	for (size_t i = 0; i < depth; i++)
		/* object, inline dynamic traits, empty class name, "a" */
		PUT(&b, AMF3_OBJECT, 0x0b, 0x01, 0x03, 'a');
	PUT(&b, AMF3_INTEGER, 0x00);
	for (size_t i = 0; i < depth; i++)
		PUT(&b, 0x01);
	return b;
}

static int decode(struct buf *b)
{
	AMFObject obj;
	int ret = AMF_Decode(&obj, b->data, (int)b->len, FALSE);
	AMF_Reset(&obj);
	free(b->data);
	b->data = NULL;
	return ret;
}

static void test_nested_object(void **state)
{
	(void)state;

	struct buf b = nested_objects(3);
	AMFObject obj;
	assert_int_equal(AMF_Decode(&obj, b.data, (int)b.len, FALSE), (int)b.len);

	/* value -> "a" -> "a" */
	assert_int_equal(AMF_CountProp(&obj), 1);
	AMFObjectProperty *prop = AMF_GetProp(&obj, NULL, 0);
	assert_int_equal(AMFProp_GetType(prop), AMF_OBJECT);
	AVal a = AVC("a");
	AMFObject level;
	for (int i = 0; i < 2; i++) {
		AMFProp_GetObject(prop, &level);
		prop = AMF_GetProp(&level, &a, -1);
		assert_int_equal(AMFProp_GetType(prop), AMF_OBJECT);
	}
	AMFProp_GetObject(prop, &level);
	assert_int_equal(AMF_CountProp(&level), 0);

	AMF_Reset(&obj);
	free(b.data);
}

static void test_depth_limit(void **state)
{
	(void)state;

	struct buf b;
	size_t len;

	b = nested_objects(MAX_DEPTH);
	len = b.len;
	assert_int_equal(decode(&b), (int)len);
	b = nested_objects(MAX_DEPTH + 1);
	assert_int_equal(decode(&b), -1);

	b = nested_arrays(MAX_DEPTH);
	len = b.len;
	assert_int_equal(decode(&b), (int)len);
	b = nested_arrays(MAX_DEPTH + 1);
	assert_int_equal(decode(&b), -1);

	b = nested_ecma_arrays(MAX_DEPTH);
	len = b.len;
	assert_int_equal(decode(&b), (int)len);
	b = nested_ecma_arrays(MAX_DEPTH + 1);
	assert_int_equal(decode(&b), -1);

	b = nested_amf3_objects(MAX_DEPTH);
	len = b.len;
	assert_int_equal(decode(&b), (int)len);
	b = nested_amf3_objects(MAX_DEPTH + 1);
	assert_int_equal(decode(&b), -1);
}

/* Deep enough to overflow any thread's stack without the limit. */
static void test_deep_nesting(void **state)
{
	(void)state;

	struct buf b;

	b = nested_objects(DEEP);
	assert_int_equal(decode(&b), -1);
	b = nested_arrays(DEEP);
	assert_int_equal(decode(&b), -1);
	b = nested_ecma_arrays(DEEP);
	assert_int_equal(decode(&b), -1);
	b = nested_amf3_objects(DEEP);
	assert_int_equal(decode(&b), -1);
}

/* An AMF3 member that fails to decode (here an unsupported array) fails the
 * whole object. librtmp used to keep the half-decoded property and step the
 * buffer back by the -1 it got (obs-rust/obs-studio#116). */
static void test_amf3_member_error(void **state)
{
	(void)state;
	/* object, inline traits, 1 sealed member, empty class name, "a",
	 * then an AMF3 array value */
	const char sealed[] = {AMF3_OBJECT, 0x13, 0x01, 0x03, 'a', AMF3_ARRAY, 0x01};
	/* object, inline externalizable traits, empty class name, then an
	 * AMF3 array value */
	const char externalizable[] = {AMF3_OBJECT, 0x07, 0x01, AMF3_ARRAY, 0x01};
	AMFObject obj;

	memset(&obj, 0, sizeof(obj));
	assert_int_equal(AMF3_Decode(&obj, sealed, (int)sizeof(sealed), TRUE), -1);
	assert_int_equal(AMF_CountProp(&obj), 0);

	memset(&obj, 0, sizeof(obj));
	assert_int_equal(AMF3_Decode(&obj, externalizable, (int)sizeof(externalizable), TRUE), -1);
	assert_int_equal(AMF_CountProp(&obj), 0);
}

/* A value that needs more bytes than the buffer has left fails to decode.
 * Each buffer here is longer than the size passed in, and the bytes past
 * that size are chosen so that a decoder which reads them would report
 * success. */
static void test_truncated_array_headers(void **state)
{
	(void)state;
	AMFObjectProperty prop;
	/* type byte, then where a 32-bit count would be */
	const char strict_array[5] = {AMF_STRICT_ARRAY, 0, 0, 0, 0};
	const char ecma_array[5] = {AMF_ECMA_ARRAY, 0, 0, 0, 0};

	assert_int_equal(AMFProp_Decode(&prop, strict_array, 1, FALSE), -1);
	assert_int_equal(AMFProp_Decode(&prop, strict_array, 4, FALSE), -1);
	assert_int_equal(AMFProp_Decode(&prop, ecma_array, 1, FALSE), -1);
	assert_int_equal(AMFProp_Decode(&prop, ecma_array, 4, FALSE), -1);

	/* with the whole count present, an empty strict array decodes */
	assert_int_equal(AMFProp_Decode(&prop, strict_array, 5, FALSE), 5);
	AMFProp_Reset(&prop);
}

/* An AMF3 string length is a 29-bit unsigned value. A length header with
 * every bit set names a string far longer than the buffer, so the property
 * fails to decode; it is not a negative length. */
static void test_amf3_string_length_is_unsigned(void **state)
{
	(void)state;
	AMFObjectProperty prop;
	const char name_all_bits[6] = {(char)0xff, (char)0xff, (char)0xff, (char)0xff, AMF3_NULL, 0};

	assert_int_equal(AMF3Prop_Decode(&prop, name_all_bits, 5, TRUE), -1);
}

/* An AMF3 integer or length header that runs past the buffer fails. */
static void test_amf3_truncated_integer(void **state)
{
	(void)state;
	AMFObjectProperty prop;
	/* AMF3 integer whose first byte says another byte follows */
	const char integer[3] = {AMF3_INTEGER, (char)0x81, 0x01};
	/* AMF3 string of 4 bytes with 1 present */
	const char string[6] = {AMF3_STRING, 0x09, 0x61, 0x61, 0x61, 0x61};

	assert_int_equal(AMF3Prop_Decode(&prop, integer, 2, FALSE), -1);
	assert_int_equal(AMF3Prop_Decode(&prop, integer, 3, FALSE), 3);
	assert_int_equal(AMF3Prop_Decode(&prop, string, 3, FALSE), -1);
	assert_int_equal(AMF3Prop_Decode(&prop, string, 6, FALSE), 6);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_nested_object),
		cmocka_unit_test(test_depth_limit),
		cmocka_unit_test(test_deep_nesting),
		cmocka_unit_test(test_amf3_member_error),
		cmocka_unit_test(test_truncated_array_headers),
		cmocka_unit_test(test_amf3_string_length_is_unsigned),
		cmocka_unit_test(test_amf3_truncated_integer),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
