/* The Windows crash handler's symbol and CPU buffers (#2819).
 *
 * libobs/obs-win-crash-handler.c.h is compiled into this test with the real
 * DbgHelp, so the cases call init_sym_info() and init_cpu_info() exactly as
 * the unhandled exception filter does. They pin:
 *
 *   - the block init_sym_info() allocates holds a SYMBOL_INFOW with
 *     MaxNameLen WCHARs of name (MaxNameLen counts WCHARs, not bytes), and
 *     SizeOfStruct names that struct;
 *   - the real SymFromAddrW resolves a function with a 200-character name
 *     into that block, and the process heap still validates afterwards;
 *   - init_cpu_info() closes the registry key it opens: calling it again and
 *     again leaves the process handle count where it was.
 *
 * The SymFromAddrW case skips when DbgHelp finds no symbols for this
 * executable (no PDB beside it). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* The crash handler's unity group gets this on the command line. */
#define OBS_VERSION "test"

#include "libobs/obs-win-crash-handler.c.h"

/* Defined in obs-windows.c.h and platform-windows.c, inside libobs and not
 * exported. */
bool sym_initialize_called = false;

const char *get_win_release_id(void)
{
	return "test";
}

/* 200 characters, one token. */
#define LONG_NAME crash_report_frame_whose_symbol_name_is_two_hundred_characters_long_abcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijab

/* An address inside the function itself: taking the function's address
 * could give an incremental-link thunk instead. */
static __declspec(noinline) DWORD64 LONG_NAME(void)
{
	struct stack_trace trace = {0};

	RtlCaptureContext(&trace.context);
	init_instruction_data(&trace);
	return trace.instruction_ptr;
}

static int setup(void **state)
{
	struct exception_handler_data *data = calloc(1, sizeof(*data));

	if (!data || !get_dbghelp_imports(data)) {
		fprintf(stderr, "DbgHelp could not be loaded\n");
		free(data);
		return -1;
	}
	data->process = GetCurrentProcess();
	*state = data;
	return 0;
}

static int teardown(void **state)
{
	struct exception_handler_data *data = *state;

	if (data->sym_info)
		data->sym_cleanup(data->process);
	exception_handler_data_free(data);
	free(data);
	return 0;
}

static void sym_info_block_holds_max_name_len_wchars(void **state)
{
	struct exception_handler_data *data = *state;

	init_sym_info(data);
	assert_non_null(data->sym_info);

	size_t size = LocalSize(data->sym_info);
	size_t needed = offsetof(SYMBOL_INFOW, Name) + data->sym_info->MaxNameLen * sizeof(WCHAR);
	if (size < needed) {
		fprintf(stderr, "sym_info block is %zu bytes; MaxNameLen %lu WCHARs need %zu\n", size,
			(unsigned long)data->sym_info->MaxNameLen, needed);
		fail();
	}
	assert_int_equal(data->sym_info->SizeOfStruct, sizeof(SYMBOL_INFOW));
}

static void sym_from_addr_resolves_a_200_character_name(void **state)
{
	struct exception_handler_data *data = *state;
	DWORD64 addr = LONG_NAME();
	DWORD64 displacement = 0;
	char name[512];

	assert_int_equal(strlen(STRINGIFY_(LONG_NAME)), 200);

	init_sym_info(data);
	assert_non_null(data->sym_info);

	if (!data->sym_from_addr(data->process, addr, &displacement, data->sym_info)) {
		fprintf(stderr, "SymFromAddrW found no symbol (error %lu): no PDB?\n", (unsigned long)GetLastError());
		skip();
	}

	assert_true(HeapValidate(GetProcessHeap(), 0, NULL));

	os_wcs_to_utf8(data->sym_info->Name, 0, name, sizeof(name));
	assert_string_equal(name, STRINGIFY_(LONG_NAME));
	assert_int_equal(data->sym_info->NameLen, 200);
}

static void cpu_info_closes_its_registry_key(void **state)
{
	struct exception_handler_data *data = *state;
	DWORD before = 0;
	DWORD after = 0;

	/* The first call may open HKEY_LOCAL_MACHINE itself, which the
	 * registry keeps open for the process. */
	init_cpu_info(data);
	dstr_free(&data->cpu_info);

	assert_true(GetProcessHandleCount(GetCurrentProcess(), &before));
	for (int i = 0; i < 32; i++) {
		init_cpu_info(data);
		assert_non_null(data->cpu_info.array);
		dstr_free(&data->cpu_info);
	}
	assert_true(GetProcessHandleCount(GetCurrentProcess(), &after));

	if (after != before) {
		fprintf(stderr, "32 calls changed the handle count from %lu to %lu\n", (unsigned long)before,
			(unsigned long)after);
		fail();
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(sym_info_block_holds_max_name_len_wchars, setup, teardown),
		cmocka_unit_test_setup_teardown(sym_from_addr_resolves_a_200_character_name, setup, teardown),
		cmocka_unit_test_setup_teardown(cpu_info_closes_its_registry_key, setup, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
