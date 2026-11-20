/* test_encoder_outputs_rust: the real private output-list adapters, bound to
 * the unchanged characterization's recording lock and unlock. One unity
 * group includes this after the test, so encoder_outputs_lock and
 * encoder_outputs_unlock are the test's own, exactly as the test's C target
 * binds the original through its pthread_mutex_lock definitions. */
#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock encoder_outputs_lock
#define pthread_mutex_unlock encoder_outputs_unlock
#include "libobs/obs-encoder-outputs-fields.c.h"
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")
