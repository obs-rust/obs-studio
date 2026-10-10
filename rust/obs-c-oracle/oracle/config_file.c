/* Test-only oracle: the original libobs/util/config-file.c, unmodified,
 * with every exported symbol renamed to oracle_* so it can link next to
 * the Rust implementation. The lexer comes from util/lexer.c, dstr from
 * util/dstr.c, bmem from test_bmem.c, blog from oracle/base.c, and the
 * os_* platform helpers from config_file_host.c and
 * file_serializer_host.c. */
#define _FILE_OFFSET_BITS 64
/* PTHREAD_MUTEX_RECURSIVE is behind _GNU_SOURCE with -std=c11. */
#define _GNU_SOURCE
/* dstr_printf is C-only and lives renamed in oracle/dstr_libc.c. */
#define dstr_printf oracle_dstr_printf
#define config_create oracle_config_create
#define config_open oracle_config_open
#define config_open_string oracle_config_open_string
#define config_open_defaults oracle_config_open_defaults
#define config_save oracle_config_save
#define config_save_safe oracle_config_save_safe
#define config_close oracle_config_close
#define config_num_sections oracle_config_num_sections
#define config_get_section oracle_config_get_section
#define config_set_string oracle_config_set_string
#define config_set_int oracle_config_set_int
#define config_set_uint oracle_config_set_uint
#define config_set_bool oracle_config_set_bool
#define config_set_double oracle_config_set_double
#define config_get_string oracle_config_get_string
#define config_get_int oracle_config_get_int
#define config_get_uint oracle_config_get_uint
#define config_get_bool oracle_config_get_bool
#define config_get_double oracle_config_get_double
#define config_remove_value oracle_config_remove_value
#define config_set_default_string oracle_config_set_default_string
#define config_set_default_int oracle_config_set_default_int
#define config_set_default_uint oracle_config_set_default_uint
#define config_set_default_bool oracle_config_set_default_bool
#define config_set_default_double oracle_config_set_default_double
#define config_get_default_string oracle_config_get_default_string
#define config_get_default_int oracle_config_get_default_int
#define config_get_default_uint oracle_config_get_default_uint
#define config_get_default_bool oracle_config_get_default_bool
#define config_get_default_double oracle_config_get_default_double
#define config_has_user_value oracle_config_has_user_value
#define config_has_default_value oracle_config_has_default_value
#define blog oracle_blog
#define blogva oracle_blogva

#include "util/config-file.c"
