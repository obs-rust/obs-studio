/* Extracted original C and discovered adapter over the real private layout. */
#include <stddef.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_service_get_signal_handler oracle_obs_service_get_signal_handler
#define obs_service_get_proc_handler oracle_obs_service_get_proc_handler
#include "obs-service-handler-getters.c.h"
#undef obs_service_get_proc_handler
#undef obs_service_get_signal_handler
#include "service_handler_getters_fields.inc"

struct service_handlers_fixture {
	struct obs_service service;
	struct obs_service before;
	/* Borrowed addresses only; align each token without a C library alignment typedef. */
	_Alignas(long double) unsigned char tokens[2][sizeof(long double)];
};

void *oracle_service_handlers_new(bool signal, bool proc, bool same, uint32_t flags);
void *oracle_service_handlers_new(bool signal, bool proc, bool same, uint32_t flags)
{
	struct service_handlers_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->service.context.signals = signal ? (void *)&fixture->tokens[0] : NULL;
		fixture->service.context.procs = proc ? (void *)&fixture->tokens[same ? 0 : 1] : NULL;
		fixture->service.active = (flags & 1) != 0;
		fixture->service.destroy = (flags & 2) != 0;
		fixture->service.owns_info_id = (flags & 4) != 0;
		memcpy(&fixture->before, &fixture->service, sizeof(fixture->service));
	}
	return fixture;
}

void oracle_service_handlers_tokens(void *handle, void **tokens);
void oracle_service_handlers_tokens(void *handle, void **tokens)
{
	struct service_handlers_fixture *fixture = handle;
	tokens[0] = &fixture->tokens[0];
	tokens[1] = &fixture->tokens[1];
}

bool oracle_service_handlers_unchanged(const void *handle);
bool oracle_service_handlers_unchanged(const void *handle)
{
	const struct service_handlers_fixture *fixture = handle;
	return memcmp(&fixture->before, &fixture->service, sizeof(fixture->service)) == 0;
}

void oracle_service_handlers_free(void *handle);
void oracle_service_handlers_free(void *handle)
{
	free(handle);
}

bool oracle_service_handlers_addresses(const obs_service_t *service);
bool oracle_service_handlers_addresses(const obs_service_t *service)
{
	struct obs_service_handler_getters_fields fields = obs_service_handler_getters_fields(service);
	return fields.signal == &service->context.signals && fields.proc == &service->context.procs;
}

void oracle_service_handlers_layout(size_t *layout);
void oracle_service_handlers_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_service_handler_getters_fields);
	layout[1] = _Alignof(struct obs_service_handler_getters_fields);
	layout[2] = offsetof(struct obs_service_handler_getters_fields, signal);
	layout[3] = offsetof(struct obs_service_handler_getters_fields, proc);
	layout[4] = sizeof(signal_handler_t *);
	layout[5] = _Alignof(signal_handler_t *);
	layout[6] = sizeof(proc_handler_t *);
	layout[7] = _Alignof(proc_handler_t *);
}

static void *service_handlers_original_signal(const obs_service_t *service)
{
	return oracle_obs_service_get_signal_handler(service);
}

static void *service_handlers_original_proc(const obs_service_t *service)
{
	return oracle_obs_service_get_proc_handler(service);
}

struct service_handlers_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void service_handlers_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void service_handlers_capture(int level, const char *format, va_list args, void *param)
{
	struct service_handlers_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void *(*service_handlers_getter)(const obs_service_t *);
typedef void (*service_handlers_get_log)(log_handler_t *, void **);
typedef void (*service_handlers_set_log)(log_handler_t, void *);
bool oracle_service_handlers_logs(service_handlers_getter signal, service_handlers_getter proc,
				  service_handlers_get_log get_log, service_handlers_set_log set_log);
bool oracle_service_handlers_logs(service_handlers_getter signal, service_handlers_getter proc,
				  service_handlers_get_log get_log, service_handlers_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	get_log(&old_handler, &old_param);
	service_handlers_getter rust[] = {signal, proc};
	service_handlers_getter original[] = {service_handlers_original_signal, service_handlers_original_proc};
	const char *expected[] = {"obs_service_get_signal_handler: Null 'service' parameter",
				  "obs_service_get_proc_handler: Null 'service' parameter"};
	bool equal = true;
	for (size_t i = 0; i < 2; i++) {
		struct service_handlers_log r = {0}, c = {0};
		set_log(service_handlers_capture, &r);
		void *rust_value = rust[i](NULL);
		set_log(service_handlers_capture, &c);
		void *c_value = original[i](NULL);
		equal = equal && !rust_value && !c_value && r.count == 1 && c.count == 1 && r.level == LOG_DEBUG &&
			c.level == LOG_DEBUG && strcmp(r.message, expected[i]) == 0 &&
			strcmp(r.message, c.message) == 0;
	}
	set_log(old_handler, old_param);
	return equal;
}
