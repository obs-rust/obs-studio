/* test_encoder_group_num_started_rust: the real private group-count
 * adapters, compiled with the unchanged characterization's own definitions
 * (pthread_mutex_lock=encoder_group_count_record_lock and the unlock), so
 * they lock and unlock through the test's recorders exactly as the C target
 * binds the original. */
#include "libobs/obs-encoder-group-num-started-fields.c.h"
