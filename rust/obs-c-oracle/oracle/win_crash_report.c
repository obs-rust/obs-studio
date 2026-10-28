/* Test-only oracle: the original libobs/obs-win-crash-report.c.h, unmodified,
 * with its nine functions renamed to oracle_* so they can link next to the
 * Rust shims. It takes no Windows type (plain integer compares and masks), so
 * it builds on every host. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define obs_win_crash_report_contains oracle_obs_win_crash_report_contains
#define obs_win_crash_report_is_crashed oracle_obs_win_crash_report_is_crashed
#define obs_win_crash_report_in_pass oracle_obs_win_crash_report_in_pass
#define obs_win_crash_report_other_process oracle_obs_win_crash_report_other_process
#define obs_win_crash_report_names_symbol oracle_obs_win_crash_report_names_symbol
#define obs_win_crash_report_mask_address oracle_obs_win_crash_report_mask_address
#define obs_win_crash_report_mask_param oracle_obs_win_crash_report_mask_param
#define obs_win_crash_report_name_prefix oracle_obs_win_crash_report_name_prefix
#define obs_win_crash_report_pass_on oracle_obs_win_crash_report_pass_on

#include "obs-win-crash-report.c.h"

#undef obs_win_crash_report_pass_on
#undef obs_win_crash_report_name_prefix
#undef obs_win_crash_report_mask_param
#undef obs_win_crash_report_mask_address
#undef obs_win_crash_report_names_symbol
#undef obs_win_crash_report_other_process
#undef obs_win_crash_report_in_pass
#undef obs_win_crash_report_is_crashed
#undef obs_win_crash_report_contains
