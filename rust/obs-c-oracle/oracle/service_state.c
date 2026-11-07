/* Independent original C state functions, compiled beside the Rust replacement. */
#define obs_service_set_destroy oracle_state_set_destroy
#define obs_service_is_destroying oracle_state_is_destroying
#define obs_service_is_active oracle_state_is_active
#define obs_service_set_active oracle_state_set_active
#define obs_service_has_output oracle_state_has_output
#define obs_service_clear_output oracle_state_clear_output
#define obs_service_detach_from_output oracle_state_detach_from_output
#define obs_service_get_control oracle_state_get_control
#include "libobs/obs-service-state.c.h"
#include "libobs/obs-service-state-adapter.c.h"
#include <stddef.h>
#include <stdlib.h>

struct state_fixture {
	struct obs_service service;
	struct obs_output output;
	struct obs_service other;
	struct obs_weak_service weak;
};
void *oracle_state_new(bool active, bool destroy, bool output, bool control, bool reverse);
void *oracle_state_new(bool active, bool destroy, bool output, bool control, bool reverse)
{
	struct state_fixture *f = calloc(1, sizeof(*f));
	f->service.active = active;
	f->service.destroy = destroy;
	f->service.output = output ? &f->output : NULL;
	f->service.context.control = control ? (obs_weak_object_t *)&f->weak : NULL;
	f->output.service = reverse ? &f->other : NULL;
	return f;
}
void oracle_state_free(void *p);
void oracle_state_free(void *p)
{
	free(p);
}
unsigned oracle_state_bits(void *p);
unsigned oracle_state_bits(void *p)
{
	struct state_fixture *f = p;
	return (unsigned)(f->service.active | (f->service.destroy << 1) | ((f->service.output != NULL) << 2) |
			  ((f->service.context.control != NULL) << 3) | ((f->output.service != NULL) << 4));
}
void *oracle_state_control(void *p);
void *oracle_state_control(void *p)
{
	return ((struct state_fixture *)p)->service.context.control;
}
long oracle_state_refs(void *p, bool weak);
long oracle_state_refs(void *p, bool weak)
{
	struct state_fixture *f = p;
	return weak ? f->weak.ref.weak_refs : f->weak.ref.refs;
}
void *oracle_state_output(void *p);
void *oracle_state_output(void *p)
{
	return ((struct state_fixture *)p)->service.output;
}
void *oracle_state_reverse(void *p);
void *oracle_state_reverse(void *p)
{
	return ((struct state_fixture *)p)->output.service;
}
size_t oracle_state_layout(unsigned index);
size_t oracle_state_layout(unsigned index)
{
	const size_t values[] = {
		sizeof(struct obs_service_state_fields),           _Alignof(struct obs_service_state_fields),
		offsetof(struct obs_service_state_fields, active), offsetof(struct obs_service_state_fields, destroy),
		offsetof(struct obs_service_state_fields, output), offsetof(struct obs_service_state_fields, control)};
	return values[index];
}
bool oracle_state_field_addresses(void *p);
bool oracle_state_field_addresses(void *p)
{
	struct state_fixture *f = p;
	struct obs_service_state_fields fields = obs_service_state_fields(&f->service);
	return fields.active == &f->service.active && fields.destroy == &f->service.destroy &&
	       fields.output == &f->service.output && fields.control == &f->service.context.control &&
	       obs_service_output_service_field(&f->output) == &f->output.service;
}
/* The lifecycle oracle formerly passed a one-byte token to a no-op detach
 * double. Its shared Rust seam now needs an actual output with its C layout. */
void *oracle_service_state_test_output(void);
void *oracle_service_state_test_output(void)
{
	static struct obs_output output;
	return &output;
}
