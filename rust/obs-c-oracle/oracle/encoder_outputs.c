/* Original obs_encoder_add_output() and obs_encoder_remove_output() and the
 * real-header adapters of obs-encoder-outputs-fields.c.h on one recording
 * fixture encoder. Under cargo test these adapters are the only definitions
 * of their real names, so the Rust shims reach this fixture. Every lock and
 * unlock of outputs_mutex is recorded with the list it saw, so a list
 * change outside the lock pair shows; a buffer reallocated by a call is
 * recorded too. The probe runs the original and a candidate pair on the
 * same operations and reports any difference in the trace or the list left
 * behind, instead of aborting, so mutants are observable. */
/* The MSVC UCRT headers define snprintf and friends inline in every object
 * that includes them, which lets a COFF link pull this member into an
 * unrelated test; keep them declarations only. */
#define _NO_CRT_STDIO_INLINE
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <string.h>

#define EO_OUTPUTS 4
#define EO_OPS 32
#define EO_TRACE 4096

/* Operation bits: the output's index (0..3), a NULL output, a NULL encoder,
 * and remove instead of add. */
#define EO_OUTPUT_MASK 0x03
#define EO_NULL_OUTPUT 0x04
#define EO_NULL_ENCODER 0x08
#define EO_REMOVE 0x10

typedef void (*eo_change_fn)(struct obs_encoder *encoder, struct obs_output *output);

static _Thread_local struct obs_encoder eo_encoder;
static _Thread_local char eo_tokens[EO_OUTPUTS];
static _Thread_local bool eo_locked;
static _Thread_local char eo_trace[EO_TRACE];
static _Thread_local size_t eo_trace_len;

static void eo_event(char value)
{
	if (eo_trace_len + 1 < sizeof(eo_trace)) {
		eo_trace[eo_trace_len++] = value;
		eo_trace[eo_trace_len] = '\0';
	}
}

static struct obs_output *eo_output(size_t index)
{
	return (struct obs_output *)&eo_tokens[index];
}

static char eo_name(const struct obs_output *output)
{
	if (!output)
		return '-';
	for (size_t i = 0; i < EO_OUTPUTS; i++) {
		if (output == eo_output(i))
			return (char)('0' + i);
	}
	return '?';
}

/* The list as the fixture holds it, then ']'. */
static void eo_snapshot(void)
{
	for (size_t i = 0; i < eo_encoder.outputs.num; i++)
		eo_event(eo_name(eo_encoder.outputs.array[i]));
	eo_event(']');
}

/* 'L' and 'U' with the list each saw; '?' for another mutex, 'R' for a lock
 * already held, 'X' for an unlock not held. */
static int eo_lock(pthread_mutex_t *mutex)
{
	eo_event('L');
	if (mutex != &eo_encoder.outputs_mutex)
		eo_event('?');
	if (eo_locked)
		eo_event('R');
	eo_locked = true;
	eo_snapshot();
	return 0;
}

static int eo_unlock(pthread_mutex_t *mutex)
{
	eo_event('U');
	if (mutex != &eo_encoder.outputs_mutex)
		eo_event('?');
	if (!eo_locked)
		eo_event('X');
	eo_locked = false;
	eo_snapshot();
	return 0;
}

void oracle_obs_encoder_add_output(struct obs_encoder *encoder, struct obs_output *output);
void oracle_obs_encoder_remove_output(struct obs_encoder *encoder, struct obs_output *output);

/* winpthreads defines the lock functions as macros: rename them to the
 * recorders only for the original and the adapters. */
#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock eo_lock
#define pthread_mutex_unlock eo_unlock
#define obs_encoder_add_output oracle_obs_encoder_add_output
#define obs_encoder_remove_output oracle_obs_encoder_remove_output
#include "libobs/obs-encoder-outputs.c.h"
#undef obs_encoder_remove_output
#undef obs_encoder_add_output
#include "encoder_outputs_fields.inc"
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")

struct eo_result {
	char trace[EO_TRACE];
	char list[EO_OPS + 2];
	size_t capacity;
};

static void eo_reset(void)
{
	memset(&eo_encoder, 0, sizeof(eo_encoder));
	eo_locked = false;
	eo_trace_len = 0;
	eo_trace[0] = '\0';
}

/* Each operation: '|', what the call recorded, 'A' when it replaced the
 * list's buffer, then 'K' if it left the mutex held. */
static void eo_run(eo_change_fn add, eo_change_fn remove, const uint8_t *ops, size_t count, struct eo_result *out)
{
	eo_reset();
	for (size_t i = 0; i < count; i++) {
		uint8_t op = ops[i];
		struct obs_encoder *encoder = (op & EO_NULL_ENCODER) ? NULL : &eo_encoder;
		struct obs_output *output = (op & EO_NULL_OUTPUT) ? NULL : eo_output(op & EO_OUTPUT_MASK);
		struct obs_output **array = eo_encoder.outputs.array;
		eo_event('|');
		if (op & EO_REMOVE)
			remove(encoder, output);
		else
			add(encoder, output);
		if (eo_encoder.outputs.array != array)
			eo_event('A');
		if (eo_locked)
			eo_event('K');
	}
	memcpy(out->trace, eo_trace, sizeof(out->trace));
	size_t n = 0;
	for (; n < eo_encoder.outputs.num && n + 1 < sizeof(out->list); n++)
		out->list[n] = eo_name(eo_encoder.outputs.array[n]);
	out->list[n] = '\0';
	out->capacity = eo_encoder.outputs.capacity;
	da_free(eo_encoder.outputs);
	eo_locked = false;
}

/* 0: match. 1: the traces differ, 2: the list or capacity left differs,
 * 0x100: bad arguments. */
unsigned oracle_encoder_outputs_probe(eo_change_fn add, eo_change_fn remove, const uint8_t *ops, size_t count);
unsigned oracle_encoder_outputs_probe(eo_change_fn add, eo_change_fn remove, const uint8_t *ops, size_t count)
{
	static _Thread_local struct eo_result original, actual;
	if (count > EO_OPS || !add || !remove)
		return 0x100;
	eo_run(oracle_obs_encoder_add_output, oracle_obs_encoder_remove_output, ops, count, &original);
	eo_run(add, remove, ops, count, &actual);
	unsigned diff = 0;
	if (strcmp(original.trace, actual.trace) != 0)
		diff |= 1;
	if (strcmp(original.list, actual.list) != 0 || original.capacity != actual.capacity)
		diff |= 2;
	return diff;
}

/* No stdio here: an oracle member defining the UCRT stdio inlines can be
 * pulled into unrelated Windows test links. */
static void eo_copy(char *out, size_t size, const char *text)
{
	if (!size)
		return;
	size_t n = strlen(text);
	if (n >= size)
		n = size - 1;
	memcpy(out, text, n);
	out[n] = '\0';
}

/* The original's trace and the list it leaves, for the characterization
 * checks. */
void oracle_encoder_outputs_original(const uint8_t *ops, size_t count, char *trace, size_t trace_size, char *list,
				     size_t list_size);
void oracle_encoder_outputs_original(const uint8_t *ops, size_t count, char *trace, size_t trace_size, char *list,
				     size_t list_size)
{
	static _Thread_local struct eo_result result;
	if (count > EO_OPS) {
		eo_copy(trace, trace_size, "");
		eo_copy(list, list_size, "");
		return;
	}
	eo_run(oracle_obs_encoder_add_output, oracle_obs_encoder_remove_output, ops, count, &result);
	eo_copy(trace, trace_size, result.trace);
	eo_copy(list, list_size, result.list);
}

/* The size of one list entry, as the real header declares it. */
size_t oracle_encoder_outputs_entry_size(void);
size_t oracle_encoder_outputs_entry_size(void)
{
	return sizeof(*eo_encoder.outputs.array);
}

/* The adapters against the real fields: the lock pair on outputs_mutex,
 * appends (a duplicate too), the count and entries, and an erase that keeps
 * the order of the rest, all through the C darray. */
bool oracle_encoder_outputs_adapters(void);
bool oracle_encoder_outputs_adapters(void)
{
	eo_reset();
	struct obs_encoder *encoder = &eo_encoder;
	bool ok = obs_encoder_outputs_count(encoder) == 0;
	obs_encoder_outputs_lock(encoder);
	obs_encoder_outputs_push(encoder, eo_output(0));
	obs_encoder_outputs_push(encoder, eo_output(1));
	obs_encoder_outputs_push(encoder, eo_output(0));
	ok = ok && obs_encoder_outputs_count(encoder) == 3 && eo_encoder.outputs.num == 3;
	ok = ok && obs_encoder_outputs_at(encoder, 0) == eo_output(0) &&
	     obs_encoder_outputs_at(encoder, 1) == eo_output(1) && obs_encoder_outputs_at(encoder, 2) == eo_output(0);
	ok = ok && eo_encoder.outputs.array[2] == eo_output(0);
	obs_encoder_outputs_erase(encoder, 0);
	ok = ok && obs_encoder_outputs_count(encoder) == 2 && obs_encoder_outputs_at(encoder, 0) == eo_output(1) &&
	     obs_encoder_outputs_at(encoder, 1) == eo_output(0);
	obs_encoder_outputs_unlock(encoder);
	ok = ok && strcmp(eo_trace, "L]U10]") == 0 && !eo_locked;
	da_free(eo_encoder.outputs);
	return ok;
}
