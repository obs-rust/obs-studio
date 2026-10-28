#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#include <setjmp.h>
#include <cmocka.h>

#include "util/dstr.h"
#include "obs-nix-system-info.h"

/*
 * Characterization of the system info parsing of libobs/obs-nix.c, moved to
 * libobs/obs-nix-system-info.c before its Rust port:
 * - obs_nix_system_info_is_name(), _is_physical_id(), _is_speed(), _value(),
 *   _physical_id() and _prints(): log_processor_info()'s /proc/cpuinfo line
 *   rules;
 * - obs_nix_system_info_is_dmesg_cpu() and _dmesg_speed():
 *   log_processor_speed()'s dmesg.boot line rules (FreeBSD);
 * - obs_nix_system_info_total_mb(), _free_mb() and _physmem_mb():
 *   log_memory_info()'s sizes (sysinfo(), and OpenBSD's HW_PHYSMEM64).
 * Saved cpuinfo and dmesg.boot text runs through a copy of each caller's
 * loop, with libobs's own dstr calls, so the blocks it would log are pinned
 * too. Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them, with libobs for the dstr calls. No test
 * runs log_system_info() itself: it reads the host's files.
 */

struct block {
	/* NULL when the name or speed was never set, as the log's %s gets */
	char *name;
	char *speed;
};

#define MAX_BLOCKS 8

struct cpuinfo_run {
	struct block blocks[MAX_BLOCKS];
	size_t count;
	/* the state at the end of the file */
	char *name;
	char *speed;
	int physical_id;
};

static char *dup_or_null(const char *s)
{
	if (!s)
		return NULL;
	char *copy = malloc(strlen(s) + 1);
	strcpy(copy, s);
	return copy;
}

/* The next line as getline() returns it: up to and including '\n', or the
 * rest of the text. */
static char *next_line(const char **text)
{
	const char *start = *text;
	if (!*start)
		return NULL;

	const char *end = strchr(start, '\n');
	size_t len = end ? (size_t)(end - start) + 1 : strlen(start);
	char *line = malloc(len + 1);
	memcpy(line, start, len);
	line[len] = '\0';
	*text = start + len;
	return line;
}

/* log_processor_info()'s loop, logging into run instead of blog(). */
static void run_cpuinfo(const char *text, struct cpuinfo_run *run)
{
	int physical_id = -1;
	int last_physical_id = -1;
	char *line;
	struct dstr proc_name;
	struct dstr proc_speed;

	memset(run, 0, sizeof(*run));
	dstr_init(&proc_name);
	dstr_init(&proc_speed);

	while ((line = next_line(&text)) != NULL) {
		if (obs_nix_system_info_is_name(line)) {
			const char *start = obs_nix_system_info_value(line);
			if (!start) {
				free(line);
				continue;
			}

			dstr_copy(&proc_name, start);
			dstr_resize(&proc_name, proc_name.len - 1);
			dstr_depad(&proc_name);
		}

		if (obs_nix_system_info_is_physical_id(line)) {
			const char *start = obs_nix_system_info_value(line);
			if (!start) {
				free(line);
				continue;
			}

			physical_id = obs_nix_system_info_physical_id(start);
		}

		if (obs_nix_system_info_is_speed(line)) {
			const char *start = obs_nix_system_info_value(line);
			if (!start) {
				free(line);
				continue;
			}

			dstr_copy(&proc_speed, start);
			dstr_resize(&proc_speed, proc_speed.len - 1);
			dstr_depad(&proc_speed);
		}

		if (obs_nix_system_info_prints(line, physical_id, last_physical_id)) {
			last_physical_id = physical_id;
			assert_true(run->count < MAX_BLOCKS);
			run->blocks[run->count].name = dup_or_null(proc_name.array);
			run->blocks[run->count].speed = dup_or_null(proc_speed.array);
			run->count++;
		}
		free(line);
	}

	run->name = dup_or_null(proc_name.array);
	run->speed = dup_or_null(proc_speed.array);
	run->physical_id = physical_id;
	dstr_free(&proc_name);
	dstr_free(&proc_speed);
}

static void free_run(struct cpuinfo_run *run)
{
	for (size_t i = 0; i < run->count; i++) {
		free(run->blocks[i].name);
		free(run->blocks[i].speed);
	}
	free(run->name);
	free(run->speed);
}

/* log_processor_speed()'s loop: the speed it would log, or NULL. */
static char *run_dmesg(const char *text)
{
	char *line;
	struct dstr proc_speed;

	dstr_init(&proc_speed);
	while ((line = next_line(&text)) != NULL) {
		if (obs_nix_system_info_is_dmesg_cpu(line)) {
			size_t len;
			const char *start = obs_nix_system_info_dmesg_speed(line, &len);
			if (!start) {
				free(line);
				continue;
			}

			dstr_ncopy(&proc_speed, start, len);
		}
		free(line);
	}

	char *speed = dup_or_null(proc_speed.array);
	dstr_free(&proc_speed);
	return speed;
}

static void assert_block(const struct block *block, const char *name, const char *speed)
{
	assert_non_null(block->name);
	assert_string_equal(block->name, name);
	assert_non_null(block->speed);
	assert_string_equal(block->speed, speed);
}

/* Two sockets, two threads each: one block per physical id. */
static const char X86_TWO_SOCKETS[] = "processor\t: 0\n"
				      "vendor_id\t: GenuineIntel\n"
				      "model name\t: Intel(R) Xeon(R) CPU E5-2680 v4 @ 2.40GHz\n"
				      "cpu MHz\t\t: 2394.454\n"
				      "physical id\t: 0\n"
				      "siblings\t: 2\n"
				      "\n"
				      "processor\t: 1\n"
				      "model name\t: Intel(R) Xeon(R) CPU E5-2680 v4 @ 2.40GHz\n"
				      "cpu MHz\t\t: 2401.000\n"
				      "physical id\t: 0\n"
				      "\n"
				      "processor\t: 2\n"
				      "model name\t: Intel(R) Xeon(R) CPU E5-2690 v4 @ 2.60GHz\n"
				      "cpu MHz\t\t: 2600.000\n"
				      "physical id\t: 1\n"
				      "\n"
				      "processor\t: 3\n"
				      "model name\t: Intel(R) Xeon(R) CPU E5-2690 v4 @ 2.60GHz\n"
				      "cpu MHz\t\t: 1200.000\n"
				      "physical id\t: 1\n"
				      "\n";

static void x86_two_sockets(void **state)
{
	(void)state;
	struct cpuinfo_run run;

	run_cpuinfo(X86_TWO_SOCKETS, &run);
	assert_int_equal(run.count, 2);
	/* each block logs the values its last line set, here the first thread
	 * of each socket */
	assert_block(&run.blocks[0], "Intel(R) Xeon(R) CPU E5-2680 v4 @ 2.40GHz", "2394.454");
	assert_block(&run.blocks[1], "Intel(R) Xeon(R) CPU E5-2690 v4 @ 2.60GHz", "2600.000");
	assert_int_equal(run.physical_id, 1);
	free_run(&run);
}

/* A single socket with no physical id line after the first block: the id
 * stays 0, so only the first block is logged. */
static void x86_one_socket(void **state)
{
	(void)state;
	struct cpuinfo_run run;

	run_cpuinfo("model name\t: AMD Ryzen 9 6900HX with Radeon Graphics\n"
		    "physical id\t: 0\n"
		    "cpu MHz\t\t: 3300.000\n"
		    "\n"
		    "model name\t: AMD Ryzen 9 6900HX with Radeon Graphics\n"
		    "physical id\t: 0\n"
		    "cpu MHz\t\t: 400.000\n"
		    "\n",
		    &run);
	assert_int_equal(run.count, 1);
	assert_block(&run.blocks[0], "AMD Ryzen 9 6900HX with Radeon Graphics", "3300.000");
	free_run(&run);
}

/* characterized, not endorsed: arm64 has no physical id and no model name,
 * so the id stays -1, the starting "last id", and nothing is logged */
static const char ARM64[] = "processor\t: 0\n"
			    "BogoMIPS\t: 48.00\n"
			    "Features\t: fp asimd evtstrm aes pmull sha1 sha2 crc32\n"
			    "CPU implementer\t: 0x41\n"
			    "CPU part\t: 0xd08\n"
			    "\n"
			    "processor\t: 1\n"
			    "BogoMIPS\t: 48.00\n"
			    "\n";

static void arm64_logs_nothing(void **state)
{
	(void)state;
	struct cpuinfo_run run;

	run_cpuinfo(ARM64, &run);
	assert_int_equal(run.count, 0);
	assert_null(run.name);
	assert_null(run.speed);
	assert_int_equal(run.physical_id, -1);
	free_run(&run);
}

/* characterized, not endorsed: a block that ends before any model name or
 * cpu MHz line logs them as NULL, which %s prints as "(null)" with glibc */
static void block_before_name(void **state)
{
	(void)state;
	struct cpuinfo_run run;

	run_cpuinfo("physical id\t: 0\n\nmodel name\t: Late\nphysical id\t: 1\ncpu MHz\t\t: 1.0\n\n", &run);
	assert_int_equal(run.count, 2);
	assert_null(run.blocks[0].name);
	assert_null(run.blocks[0].speed);
	assert_block(&run.blocks[1], "Late", "1.0");
	free_run(&run);
}

/* characterized, not endorsed: the caller cuts the last byte of every value
 * as its newline, so a value on the last line of a file with no newline
 * loses its last real byte (it is never logged: no blank line follows) */
static void last_line_without_newline(void **state)
{
	(void)state;
	struct cpuinfo_run run;

	run_cpuinfo("physical id\t: 0\ncpu MHz\t\t: 1234.567\nmodel name\t: Name", &run);
	assert_int_equal(run.count, 0);
	assert_string_equal(run.name, "Nam");
	assert_string_equal(run.speed, "1234.567");
	free_run(&run);

	run_cpuinfo("model name\t: Name\ncpu MHz\t\t: 1234.567", &run);
	assert_string_equal(run.name, "Name");
	assert_string_equal(run.speed, "1234.56");
	free_run(&run);
}

static void key_prefixes(void **state)
{
	(void)state;

	assert_true(obs_nix_system_info_is_name("model name\t: x\n"));
	assert_true(obs_nix_system_info_is_name("model name"));
	/* compared by the key's length, so a longer key matches */
	assert_true(obs_nix_system_info_is_name("model names\t: x\n"));
	assert_true(obs_nix_system_info_is_name("model namex"));
	assert_false(obs_nix_system_info_is_name("model nam"));
	assert_false(obs_nix_system_info_is_name("Model name\t: x\n"));
	assert_false(obs_nix_system_info_is_name(" model name\t: x\n"));
	assert_false(obs_nix_system_info_is_name("\n"));
	assert_false(obs_nix_system_info_is_name(""));

	assert_true(obs_nix_system_info_is_physical_id("physical id\t: 0\n"));
	assert_true(obs_nix_system_info_is_physical_id("physical ids\t: 0\n"));
	assert_false(obs_nix_system_info_is_physical_id("physical i"));
	assert_false(obs_nix_system_info_is_physical_id("physical_id\t: 0\n"));

	assert_true(obs_nix_system_info_is_speed("cpu MHz\t\t: 1.0\n"));
	assert_true(obs_nix_system_info_is_speed("cpu MHz dynamic\t: 1.0\n"));
	assert_false(obs_nix_system_info_is_speed("cpu MH"));
	assert_false(obs_nix_system_info_is_speed("CPU MHz\t\t: 1.0\n"));
	assert_false(obs_nix_system_info_is_speed("cpu mhz\t\t: 1.0\n"));

	/* the keys never overlap */
	assert_false(obs_nix_system_info_is_physical_id("model name\t: x\n"));
	assert_false(obs_nix_system_info_is_speed("model name\t: x\n"));
	assert_false(obs_nix_system_info_is_name("cpu MHz\t\t: 1.0\n"));
}

static void values(void **state)
{
	(void)state;
	const char *line;

	line = "model name\t: Intel\n";
	assert_ptr_equal(obs_nix_system_info_value(line), line + 12);
	/* the first ':', not the last */
	line = "model name\t: a:b\n";
	assert_ptr_equal(obs_nix_system_info_value(line), line + 12);
	line = "model name:x";
	assert_ptr_equal(obs_nix_system_info_value(line), line + 11);
	/* no ':' */
	assert_null(obs_nix_system_info_value("model name\n"));
	assert_null(obs_nix_system_info_value(""));
	/* a line that ends at the ':' (the last line, with no newline) */
	assert_null(obs_nix_system_info_value("model name\t:"));
	/* characterized, not endorsed: "model name\t:\n" has the newline as
	 * its value, so the caller cuts it to nothing and dstr_resize() frees
	 * the name */
	line = "model name\t:\n";
	assert_ptr_equal(obs_nix_system_info_value(line), line + 12);

	struct cpuinfo_run run;
	run_cpuinfo("model name\t: Old\nmodel name\t:\nphysical id\t: 0\n\n", &run);
	assert_int_equal(run.count, 1);
	assert_null(run.blocks[0].name);
	free_run(&run);

	/* "model name\t:" at the end of the file keeps the old name */
	run_cpuinfo("model name\t: Old\nmodel name\t:", &run);
	assert_string_equal(run.name, "Old");
	free_run(&run);

	/* depadded on both ends after the cut */
	run_cpuinfo("model name\t:  \t Padded  Name \t\nphysical id\t: 3\ncpu MHz\t:\t 800.0  \n\n", &run);
	assert_int_equal(run.count, 1);
	assert_block(&run.blocks[0], "Padded  Name", "800.0");
	free_run(&run);
}

static void physical_ids(void **state)
{
	(void)state;

	assert_int_equal(obs_nix_system_info_physical_id(" 0\n"), 0);
	assert_int_equal(obs_nix_system_info_physical_id(" 1\n"), 1);
	assert_int_equal(obs_nix_system_info_physical_id("\t 12\n"), 12);
	assert_int_equal(obs_nix_system_info_physical_id(" -3\n"), -3);
	assert_int_equal(obs_nix_system_info_physical_id(" +7\n"), 7);
	assert_int_equal(obs_nix_system_info_physical_id(" 42abc\n"), 42);
	assert_int_equal(obs_nix_system_info_physical_id(" 2147483647\n"), INT_MAX);
	assert_int_equal(obs_nix_system_info_physical_id(" -2147483648\n"), INT_MIN);
	/* not a number */
	assert_int_equal(obs_nix_system_info_physical_id(" x\n"), 0);
	assert_int_equal(obs_nix_system_info_physical_id(" - 1\n"), 0);
	assert_int_equal(obs_nix_system_info_physical_id("\n"), 0);
	assert_int_equal(obs_nix_system_info_physical_id(" 0x10\n"), 0);

	/* characterized, not endorsed: "physical id : x" reads as 0, so a
	 * block after it is logged as socket 0 */
	struct cpuinfo_run run;
	run_cpuinfo("physical id : x\nmodel name : X\n\n", &run);
	assert_int_equal(run.count, 1);
	assert_int_equal(run.physical_id, 0);
	free_run(&run);

	/* a "physical id" line with no value keeps the old id */
	run_cpuinfo("physical id\t: 5\nphysical id\t:", &run);
	assert_int_equal(run.physical_id, 5);
	free_run(&run);
}

static void print_rule(void **state)
{
	(void)state;

	assert_true(obs_nix_system_info_prints("\n", 0, -1));
	assert_true(obs_nix_system_info_prints("\n", 1, 0));
	assert_true(obs_nix_system_info_prints("\n", -1, 0));
	assert_true(obs_nix_system_info_prints("\n", INT_MIN, INT_MAX));
	assert_false(obs_nix_system_info_prints("\n", -1, -1));
	assert_false(obs_nix_system_info_prints("\n", 0, 0));
	/* only a line that starts with the newline */
	assert_false(obs_nix_system_info_prints(" \n", 0, -1));
	assert_false(obs_nix_system_info_prints("\r\n", 0, -1));
	assert_false(obs_nix_system_info_prints("\t\n", 0, -1));
	assert_false(obs_nix_system_info_prints("", 0, -1));
	assert_false(obs_nix_system_info_prints("x\n", 0, -1));
	assert_true(obs_nix_system_info_prints("\nx", 0, -1));
}

static void dmesg_lines(void **state)
{
	(void)state;
	size_t len = 99;
	const char *line;

	assert_true(obs_nix_system_info_is_dmesg_cpu("CPU: AMD\n"));
	assert_true(obs_nix_system_info_is_dmesg_cpu("CPU: "));
	assert_false(obs_nix_system_info_is_dmesg_cpu("CPU:"));
	assert_false(obs_nix_system_info_is_dmesg_cpu("CPU:x"));
	assert_false(obs_nix_system_info_is_dmesg_cpu("cpu: x"));
	assert_false(obs_nix_system_info_is_dmesg_cpu(" CPU: x"));

	line = "CPU: AMD Ryzen 7 5800X 8-Core Processor (3800.11-MHz K8-class CPU)\n";
	assert_ptr_equal(obs_nix_system_info_dmesg_speed(line, &len), strchr(line, '(') + 1);
	assert_int_equal(len, 7);
	/* the last '(' */
	line = "CPU: a (1-b (2-c\n";
	assert_ptr_equal(obs_nix_system_info_dmesg_speed(line, &len), line + 13);
	assert_int_equal(len, 1);
	/* no '(' leaves len untouched */
	len = 99;
	assert_null(obs_nix_system_info_dmesg_speed("CPU: none\n", &len));
	assert_int_equal(len, 99);
	/* a line that ends at the '(' */
	assert_null(obs_nix_system_info_dmesg_speed("CPU: x (", &len));
	assert_int_equal(len, 99);
	/* characterized, not endorsed: with no '-', the rest of the line is the
	 * speed, its newline and ')' included */
	line = "CPU: x (500)\n";
	assert_ptr_equal(obs_nix_system_info_dmesg_speed(line, &len), line + 8);
	assert_int_equal(len, 5);
	line = "CPU: x (\n";
	assert_ptr_equal(obs_nix_system_info_dmesg_speed(line, &len), line + 8);
	assert_int_equal(len, 1);
	/* a '-' first gives an empty speed */
	line = "CPU: x (-MHz)\n";
	assert_ptr_equal(obs_nix_system_info_dmesg_speed(line, &len), line + 8);
	assert_int_equal(len, 0);

	char *speed = run_dmesg("Copyright (c) 1992-2023 The FreeBSD Project.\n"
				"CPU: AMD Ryzen 7 5800X 8-Core Processor (3800.11-MHz K8-class CPU)\n"
				"  Origin=\"AuthenticAMD\"  Id=0xa20f12\n");
	assert_string_equal(speed, "3800.11");
	free(speed);

	/* the last CPU line with a speed wins; an empty one clears it */
	speed = run_dmesg("CPU: A (1000.00-MHz)\nCPU: B (2000.00-MHz)\nCPU: none\n");
	assert_string_equal(speed, "2000.00");
	free(speed);
	speed = run_dmesg("CPU: A (1000.00-MHz)\nCPU: B (-MHz)\n");
	assert_null(speed);
	free(speed);
	speed = run_dmesg("Copyright (c) 1992-2023\n");
	assert_null(speed);
}

static void memory_sizes(void **state)
{
	(void)state;

	/* mem_unit 1 (64-bit kernels) */
	assert_int_equal(obs_nix_system_info_total_mb(17179869184ull, 1), 16384);
	assert_int_equal(obs_nix_system_info_total_mb(1048575, 1), 0);
	assert_int_equal(obs_nix_system_info_total_mb(1048576, 1), 1);
	assert_int_equal(obs_nix_system_info_free_mb(1048576, 1048576, 1), 2);
	assert_int_equal(obs_nix_system_info_free_mb(524288, 524287, 1), 0);
	assert_int_equal(obs_nix_system_info_free_mb(524288, 524288, 1), 1);
	/* mem_unit 4096 (pages), totalram near 2^32 */
	assert_int_equal(obs_nix_system_info_total_mb(UINT32_MAX, 4096), 16777215);
	assert_int_equal(obs_nix_system_info_total_mb(4294967296ull, 4096), 16777216);
	assert_int_equal(obs_nix_system_info_total_mb(256, 4096), 1);
	assert_int_equal(obs_nix_system_info_total_mb(255, 4096), 0);
	assert_int_equal(obs_nix_system_info_free_mb(UINT32_MAX, UINT32_MAX, 4096), 33554431);
	assert_int_equal(obs_nix_system_info_free_mb(128, 128, 4096), 1);
	/* mem_unit 0 */
	assert_int_equal(obs_nix_system_info_total_mb(UINT64_MAX, 0), 0);
	assert_int_equal(obs_nix_system_info_free_mb(UINT64_MAX, UINT64_MAX, 0), 0);
	/* characterized, not endorsed: the products wrap in uint64_t (only
	 * above 16 EiB) */
	assert_int_equal(obs_nix_system_info_total_mb(1ull << 63, 2), 0);
	assert_int_equal(obs_nix_system_info_total_mb(UINT64_MAX, UINT32_MAX), 17592186040320ull);
	assert_int_equal(obs_nix_system_info_free_mb(UINT64_MAX, 1, 4096), 0);
	assert_int_equal(obs_nix_system_info_free_mb(UINT64_MAX, 2, 1), 0);

	/* OpenBSD: signed, truncated toward zero */
	assert_int_equal(obs_nix_system_info_physmem_mb(17179869184ll), 16384);
	assert_int_equal(obs_nix_system_info_physmem_mb(0), 0);
	assert_int_equal(obs_nix_system_info_physmem_mb(1048575), 0);
	assert_int_equal(obs_nix_system_info_physmem_mb(-1), 0);
	assert_int_equal(obs_nix_system_info_physmem_mb(-1048576), -1);
	assert_int_equal(obs_nix_system_info_physmem_mb(-2097151), -1);
	assert_int_equal(obs_nix_system_info_physmem_mb(INT64_MAX), 8796093022207ll);
	assert_int_equal(obs_nix_system_info_physmem_mb(INT64_MIN), -8796093022208ll);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(x86_two_sockets),
		cmocka_unit_test(x86_one_socket),
		cmocka_unit_test(arm64_logs_nothing),
		cmocka_unit_test(block_before_name),
		cmocka_unit_test(last_line_without_newline),
		cmocka_unit_test(key_prefixes),
		cmocka_unit_test(values),
		cmocka_unit_test(physical_ids),
		cmocka_unit_test(print_rule),
		cmocka_unit_test(dmesg_lines),
		cmocka_unit_test(memory_sizes),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
