#include "libobs/build/libobs-graphics.pch.h"

/* Private adapters for header-inline operations, not a new public API.
 * Collection indices match obs_graphics::effect_parser_lifecycle::Collection.
 * Rust owns traversal/order and top-level storage release. */
void obs_effect_parser_destroy_element(struct effect_parser *ep, size_t collection, size_t index);
void obs_effect_parser_destroy_state(struct effect_parser *ep);
void obs_effect_parser_release_storage(struct effect_parser *ep, size_t collection);

void obs_effect_parser_destroy_element(struct effect_parser *ep, size_t collection, size_t index)
{
	switch (collection) {
	case 0:
		ep_param_free(ep->params.array + index);
		break;
	case 1:
		ep_struct_free(ep->structs.array + index);
		break;
	case 2:
		ep_func_free(ep->funcs.array + index);
		break;
	case 3:
		ep_sampler_free(ep->samplers.array + index);
		break;
	case 4:
		ep_technique_free(ep->techniques.array + index);
		break;
	default:
		assert(false);
	}
}

void obs_effect_parser_destroy_state(struct effect_parser *ep)
{
	cf_parser_free(&ep->cfp);
}

void obs_effect_parser_release_storage(struct effect_parser *ep, size_t collection)
{
	switch (collection) {
	case 0:
		da_free(ep->params);
		break;
	case 1:
		da_free(ep->structs);
		break;
	case 2:
		da_free(ep->funcs);
		break;
	case 3:
		da_free(ep->samplers);
		break;
	case 4:
		da_free(ep->techniques);
		break;
	default:
		assert(false);
	}
}
