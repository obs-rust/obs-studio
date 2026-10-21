/* Test-only oracle: libobs/util/darray.h is header-only (static inline), so
 * this file exposes non-inline oracle_* wrappers that call the real header
 * functions, plus the struct layout as the C compiler sees it. */
#include <stddef.h>
#include <stdint.h>

#include "util/darray.h"

/* Item types for the da_push_back_new macro, which needs a typed DARRAY:
 * 1, 4, 12 and 16 bytes (the last shaped like struct encoder_packet_time). */
struct oracle_item12 {
	uint32_t v[3];
};
struct oracle_item16 {
	int64_t pts;
	uint32_t track;
	uint32_t tag;
};

/* Test-only exports called from Rust over FFI. */
void oracle_darray_free(struct darray *da);
void oracle_darray_reserve(size_t es, struct darray *da, size_t capacity);
void oracle_darray_ensure_capacity(size_t es, struct darray *da, size_t new_size);
void oracle_darray_resize(size_t es, struct darray *da, size_t size);
void oracle_darray_clear(struct darray *da);
size_t oracle_darray_push_back_array(size_t es, struct darray *da, const void *array, size_t num);
void oracle_darray_erase(size_t es, struct darray *da, size_t idx);
void oracle_darray_pop_back(size_t es, struct darray *da);
size_t oracle_darray_push_back(size_t es, struct darray *da, const void *item);
void *oracle_darray_push_back_new(size_t es, struct darray *da);
void *oracle_da_push_back_new(size_t es, struct darray *da);
void oracle_darray_insert(size_t es, struct darray *da, size_t idx, const void *item);
void oracle_darray_erase_range(size_t es, struct darray *da, size_t start, size_t end);
void oracle_darray_pop_front(size_t es, struct darray *da);
size_t oracle_darray_size(void);
size_t oracle_darray_align(void);
size_t oracle_darray_offset_array(void);
size_t oracle_darray_offset_num(void);
size_t oracle_darray_offset_capacity(void);

void oracle_darray_free(struct darray *da)
{
	darray_free(da);
}
void oracle_darray_reserve(size_t es, struct darray *da, size_t capacity)
{
	darray_reserve(es, da, capacity);
}
void oracle_darray_ensure_capacity(size_t es, struct darray *da, size_t new_size)
{
	darray_ensure_capacity(es, da, new_size);
}
void oracle_darray_resize(size_t es, struct darray *da, size_t size)
{
	darray_resize(es, da, size);
}
void oracle_darray_clear(struct darray *da)
{
	darray_clear(da);
}
size_t oracle_darray_push_back_array(size_t es, struct darray *da, const void *array, size_t num)
{
	return darray_push_back_array(es, da, array, num);
}
void oracle_darray_erase(size_t es, struct darray *da, size_t idx)
{
	darray_erase(es, da, idx);
}
void oracle_darray_pop_back(size_t es, struct darray *da)
{
	darray_pop_back(es, da);
}
size_t oracle_darray_push_back(size_t es, struct darray *da, const void *item)
{
	return darray_push_back(es, da, item);
}
void *oracle_darray_push_back_new(size_t es, struct darray *da)
{
	return darray_push_back_new(es, da);
}

/* The da_push_back_new macro on a DARRAY(type) that aliases *da. With GCC
 * and Clang it is a statement expression of its own; elsewhere it is
 * darray_push_back_new. Returns NULL for an element size it has no type
 * for. */
#define ORACLE_DA_PUSH_BACK_NEW(type)      \
	do {                               \
		DARRAY(type) v;            \
		v.da = *da;                \
		item = da_push_back_new(v); \
		*da = v.da;                \
	} while (0)

void *oracle_da_push_back_new(size_t es, struct darray *da)
{
	void *item = NULL;
	switch (es) {
	case 1:
		ORACLE_DA_PUSH_BACK_NEW(uint8_t);
		break;
	case 4:
		ORACLE_DA_PUSH_BACK_NEW(uint32_t);
		break;
	case sizeof(struct oracle_item12):
		ORACLE_DA_PUSH_BACK_NEW(struct oracle_item12);
		break;
	case sizeof(struct oracle_item16):
		ORACLE_DA_PUSH_BACK_NEW(struct oracle_item16);
		break;
	}
	return item;
}

void oracle_darray_insert(size_t es, struct darray *da, size_t idx, const void *item)
{
	darray_insert(es, da, idx, item);
}
void oracle_darray_erase_range(size_t es, struct darray *da, size_t start, size_t end)
{
	darray_erase_range(es, da, start, end);
}
void oracle_darray_pop_front(size_t es, struct darray *da)
{
	darray_pop_front(es, da);
}

/* Layout of struct darray. */
size_t oracle_darray_size(void)
{
	return sizeof(struct darray);
}
size_t oracle_darray_align(void)
{
	return _Alignof(struct darray);
}
size_t oracle_darray_offset_array(void)
{
	return offsetof(struct darray, array);
}
size_t oracle_darray_offset_num(void)
{
	return offsetof(struct darray, num);
}
size_t oracle_darray_offset_capacity(void)
{
	return offsetof(struct darray, capacity);
}
