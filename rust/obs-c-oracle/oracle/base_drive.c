/* Drivers for the base parity test. They build a real va_list in C and call
 * either the Rust-backed adapter (blog / blogva) or the original oracle. */
/* The parity tests choose the format string at run time (they drive blog
 * with arbitrary formats), so these test entry points forward non-literal
 * formats by design (obs-rust/obs-studio#516). */
#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic ignored "-Wformat-nonliteral"
#endif

#include <stdarg.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>

#if defined(_MSC_VER)
/* MSVC's <stdatomic.h> errors unless C11 atomics are switched on
 * (vcruntime_c11_stdatomic.h: "C atomic support is not enabled").
 * The owner uses the same intrinsics as util/threading-windows.h. */
#include <intrin.h>
#else
#include <stdatomic.h>
#endif
#include <util/base.h>

struct base_test_rec {
	int calls;
	int level;
	char message[256];
};

/* Test-only exports called from Rust over FFI. */
void base_test_record(int log_level, const char *format, va_list args, void *param);
log_handler_t base_test_record_ptr(void);
void base_test_record_only_this_thread(void);
void base_test_flush(void);
void base_test_rust_blog_i(int level, const char *fmt, int v);
void base_test_oracle_blog_i(int level, const char *fmt, int v);
void base_test_rust_blog_s(int level, const char *fmt, const char *s);
void base_test_oracle_blog_s(int level, const char *fmt, const char *s);
void base_test_rust_blog_ss(int level, const char *fmt, const char *a, const char *b);
void base_test_oracle_blog_ss(int level, const char *fmt, const char *a, const char *b);
void base_test_rust_blog_0(int level, const char *fmt);
void base_test_oracle_blog_0(int level, const char *fmt);
void base_test_rust_blogva_si(int level, const char *fmt, const char *s, int v);
void base_test_oracle_blogva_si(int level, const char *fmt, const char *s, int v);

/* Off unless a test binds an owner. base_parity records every thread.
 * proc_parity, delay_parity and the effect parity tests bind the test
 * thread: other tests in their binaries blog through the same handler,
 * and their lines must not change the count.
 * The owner is never cleared: blog calls the handler after it drops the
 * base mutex, so a line can arrive after the old handler is restored.
 * The owner is an atomic numeric token. TLS addresses can be reused after
 * thread exit, so each thread lazily claims a process-unique token instead.
 * Zero stays reserved for the unbound, record-all state. */
#if defined(_MSC_VER)
static __declspec(thread) uint64_t thread_token;
static volatile __int64 record_owner;
static volatile __int64 next_thread_token;

static uint64_t record_owner_load(void)
{
	return (uint64_t)_InterlockedCompareExchange64(&record_owner, 0, 0);
}

static void record_owner_store(uint64_t owner)
{
	_InterlockedExchange64(&record_owner, (__int64)owner);
}

static uint64_t allocate_thread_token(void)
{
	__int64 previous = _InterlockedCompareExchange64(&next_thread_token, 0, 0);
	for (;;) {
		if ((uint64_t)previous == UINT64_MAX)
			abort();
		__int64 next = (__int64)((uint64_t)previous + 1);
		__int64 observed = _InterlockedCompareExchange64(&next_thread_token, next, previous);
		if (observed == previous)
			return (uint64_t)next;
		previous = observed;
	}
}
#else
static _Thread_local uint64_t thread_token;
static _Atomic(uint64_t) record_owner;
static _Atomic(uint64_t) next_thread_token;

static uint64_t record_owner_load(void)
{
	return atomic_load_explicit(&record_owner, memory_order_acquire);
}

static void record_owner_store(uint64_t owner)
{
	atomic_store_explicit(&record_owner, owner, memory_order_release);
}

static uint64_t allocate_thread_token(void)
{
	uint64_t previous = atomic_load_explicit(&next_thread_token, memory_order_relaxed);
	for (;;) {
		if (previous == UINT64_MAX)
			abort();
		if (atomic_compare_exchange_weak_explicit(&next_thread_token, &previous, previous + 1,
							  memory_order_relaxed, memory_order_relaxed))
			return previous + 1;
	}
}
#endif

static uint64_t recorder_thread_token(void)
{
	if (!thread_token) {
		thread_token = allocate_thread_token();
	}
	return thread_token;
}

void base_test_record_only_this_thread(void)
{
	record_owner_store(recorder_thread_token());
}

void base_test_record(int log_level, const char *format, va_list args, void *param)
{
	struct base_test_rec *rec = param;
	uint64_t owner = record_owner_load();

	if (owner && owner != recorder_thread_token())
		return;

	rec->calls++;
	rec->level = log_level;
	vsnprintf(rec->message, sizeof(rec->message), format, args);
}

log_handler_t base_test_record_ptr(void)
{
	return base_test_record;
}

void base_test_flush(void)
{
	fflush(stdout);
	fflush(stderr);
}

void oracle_blog(int log_level, const char *format, ...);
void oracle_blogva(int log_level, const char *format, va_list args);

static void call_va(void (*fn)(int, const char *, va_list), int level, const char *fmt, ...)
{
	va_list args;

	va_start(args, fmt);
	fn(level, fmt, args);
	va_end(args);
}

void base_test_rust_blog_i(int level, const char *fmt, int v)
{
	blog(level, fmt, v);
}

void base_test_oracle_blog_i(int level, const char *fmt, int v)
{
	oracle_blog(level, fmt, v);
}

void base_test_rust_blog_s(int level, const char *fmt, const char *s)
{
	blog(level, fmt, s);
}

void base_test_oracle_blog_s(int level, const char *fmt, const char *s)
{
	oracle_blog(level, fmt, s);
}

void base_test_rust_blog_ss(int level, const char *fmt, const char *a, const char *b)
{
	blog(level, fmt, a, b);
}

void base_test_oracle_blog_ss(int level, const char *fmt, const char *a, const char *b)
{
	oracle_blog(level, fmt, a, b);
}

void base_test_rust_blog_0(int level, const char *fmt)
{
	/* blog() is PRINTFATTR, so a runtime format with no extra args trips
	 * -Wformat-security. blogva is the same call blog makes. */
	call_va(blogva, level, fmt);
}

void base_test_oracle_blog_0(int level, const char *fmt)
{
	oracle_blog(level, fmt);
}

void base_test_rust_blogva_si(int level, const char *fmt, const char *s, int v)
{
	call_va(blogva, level, fmt, s, v);
}

void base_test_oracle_blogva_si(int level, const char *fmt, const char *s, int v)
{
	call_va(oracle_blogva, level, fmt, s, v);
}
