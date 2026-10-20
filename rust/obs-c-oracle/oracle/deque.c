/* Test-only oracle: libobs/util/deque.h is header-only (static inline), so
 * this file exposes non-inline oracle_* wrappers that call the real header
 * functions, plus the struct layout as the C compiler sees it. */
#include <stddef.h>

#include "util/deque.h"

/* Test-only exports called from Rust over FFI. */
void oracle_deque_init(struct deque *dq);
void oracle_deque_free(struct deque *dq);
void oracle_deque_reserve(struct deque *dq, size_t capacity);
void oracle_deque_upsize(struct deque *dq, size_t size);
void oracle_deque_place(struct deque *dq, size_t position, const void *data, size_t size);
void oracle_deque_push_back(struct deque *dq, const void *data, size_t size);
void oracle_deque_push_front(struct deque *dq, const void *data, size_t size);
void oracle_deque_push_back_zero(struct deque *dq, size_t size);
void oracle_deque_push_front_zero(struct deque *dq, size_t size);
void oracle_deque_peek_front(struct deque *dq, void *data, size_t size);
void oracle_deque_peek_back(struct deque *dq, void *data, size_t size);
void oracle_deque_pop_front(struct deque *dq, void *data, size_t size);
void oracle_deque_pop_back(struct deque *dq, void *data, size_t size);
void *oracle_deque_data(struct deque *dq, size_t idx);
size_t oracle_deque_size(void);
size_t oracle_deque_align(void);
size_t oracle_deque_offset_data(void);
size_t oracle_deque_offset_size(void);
size_t oracle_deque_offset_start_pos(void);
size_t oracle_deque_offset_end_pos(void);
size_t oracle_deque_offset_capacity(void);

void oracle_deque_init(struct deque *dq)
{
	deque_init(dq);
}
void oracle_deque_free(struct deque *dq)
{
	deque_free(dq);
}
void oracle_deque_reserve(struct deque *dq, size_t capacity)
{
	deque_reserve(dq, capacity);
}
void oracle_deque_upsize(struct deque *dq, size_t size)
{
	deque_upsize(dq, size);
}
void oracle_deque_place(struct deque *dq, size_t position, const void *data, size_t size)
{
	deque_place(dq, position, data, size);
}
void oracle_deque_push_back(struct deque *dq, const void *data, size_t size)
{
	deque_push_back(dq, data, size);
}
void oracle_deque_push_front(struct deque *dq, const void *data, size_t size)
{
	deque_push_front(dq, data, size);
}
void oracle_deque_push_back_zero(struct deque *dq, size_t size)
{
	deque_push_back_zero(dq, size);
}
void oracle_deque_push_front_zero(struct deque *dq, size_t size)
{
	deque_push_front_zero(dq, size);
}
void oracle_deque_peek_front(struct deque *dq, void *data, size_t size)
{
	deque_peek_front(dq, data, size);
}
void oracle_deque_peek_back(struct deque *dq, void *data, size_t size)
{
	deque_peek_back(dq, data, size);
}
void oracle_deque_pop_front(struct deque *dq, void *data, size_t size)
{
	deque_pop_front(dq, data, size);
}
void oracle_deque_pop_back(struct deque *dq, void *data, size_t size)
{
	deque_pop_back(dq, data, size);
}
void *oracle_deque_data(struct deque *dq, size_t idx)
{
	return deque_data(dq, idx);
}

/* Layout of struct deque. */
size_t oracle_deque_size(void)
{
	return sizeof(struct deque);
}
size_t oracle_deque_align(void)
{
	return _Alignof(struct deque);
}
size_t oracle_deque_offset_data(void)
{
	return offsetof(struct deque, data);
}
size_t oracle_deque_offset_size(void)
{
	return offsetof(struct deque, size);
}
size_t oracle_deque_offset_start_pos(void)
{
	return offsetof(struct deque, start_pos);
}
size_t oracle_deque_offset_end_pos(void)
{
	return offsetof(struct deque, end_pos);
}
size_t oracle_deque_offset_capacity(void)
{
	return offsetof(struct deque, capacity);
}
