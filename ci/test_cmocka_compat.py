#!/usr/bin/env -S uv run --script
"""The cmocka tests build against cmocka 1.1 (#2197).

    uv run python -m unittest ci/test_cmocka_compat.py

Debian 13 and Ubuntu 24.04/25.10 ship cmocka 1.1.7; CI's Ubuntu 26.04 has
2.0.2, so a test that uses a 2.x-only name builds in CI and fails there. A
test may use such a name only through test/cmocka/cmocka-compat.h, which
maps it to the 1.1 API when cmocka does not define it.

CMOCKA_2_ONLY is the public names of the cmocka 2.0.2 cmocka.h that the
1.1.7 cmocka.h does not have. Not checked here: a C test that uses
true/false must reach <stdbool.h> itself (the 2.x cmocka.h includes it,
1.1.7 does not); only a build against 1.1.7 shows that reliably.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CMOCKA = ROOT / "test" / "cmocka"
COMPAT = CMOCKA / "cmocka-compat.h"
COMPAT_INCLUDE = re.compile(r'^\s*#\s*include\s*"cmocka-compat\.h"', re.M)
COMMENT = re.compile(r"/\*.*?\*/|//[^\n]*", re.S)

CMOCKA_2_ONLY = frozenset(
    """
    assert_float_in_range assert_float_in_set assert_float_not_in_range
    assert_float_not_in_set assert_int_in_range assert_int_in_set
    assert_int_not_in_range assert_int_not_in_set assert_non_null_msg
    assert_null_msg assert_ptr_equal_msg assert_ptr_not_equal_msg
    assert_uint_equal assert_uint_in_range assert_uint_in_set
    assert_uint_not_equal assert_uint_not_in_range assert_uint_not_in_set
    cast_int_to_cmocka_value cast_ptr_to_cmocka_value
    cast_ptr_to_uintmax_type cast_to_double_type cast_to_float_type
    cast_to_intmax_type cast_to_uintmax_type cast_to_void_pointer
    check_expected_any check_expected_double check_expected_float
    check_expected_int check_expected_uint cmocka_print_error
    cmocka_set_callbacks expect_check_count expect_check_data
    expect_check_data_count expect_double expect_double_count expect_float
    expect_float_count expect_float_in_range expect_float_in_range_count
    expect_float_in_set expect_float_in_set_count expect_float_not_in_range
    expect_float_not_in_range_count expect_float_not_in_set
    expect_float_not_in_set_count expect_int_in_range
    expect_int_in_range_count expect_int_in_set expect_int_in_set_count
    expect_int_not_in_range expect_int_not_in_range_count
    expect_int_not_in_set expect_int_not_in_set_count expect_int_not_value
    expect_int_not_value_count expect_int_value expect_int_value_count
    expect_not_double expect_not_double_count expect_not_float
    expect_not_float_count expect_uint_in_range expect_uint_in_range_count
    expect_uint_in_set expect_uint_in_set_count expect_uint_not_in_range
    expect_uint_not_in_range_count expect_uint_not_in_set
    expect_uint_not_in_set_count expect_uint_not_value
    expect_uint_not_value_count expect_uint_value expect_uint_value_count
    mock_double mock_errno mock_float mock_int mock_parameter
    mock_parameter_double mock_parameter_float mock_parameter_int
    mock_parameter_ptr mock_parameter_ptr_type mock_parameter_type
    mock_parameter_uint mock_ptr_type_checked mock_uint will_return_double
    will_return_double_always will_return_double_count
    will_return_double_maybe will_return_float will_return_float_always
    will_return_float_count will_return_float_maybe will_return_int
    will_return_int_always will_return_int_count will_return_int_maybe
    will_return_ptr will_return_ptr_always will_return_ptr_count
    will_return_ptr_maybe will_return_ptr_type will_return_uint
    will_return_uint_always will_return_uint_count will_return_uint_maybe
    will_set_errno will_set_errno_always will_set_errno_count
    will_set_errno_maybe will_set_parameter will_set_parameter_always
    will_set_parameter_count will_set_parameter_double
    will_set_parameter_double_always will_set_parameter_double_count
    will_set_parameter_double_maybe will_set_parameter_float
    will_set_parameter_float_always will_set_parameter_float_count
    will_set_parameter_float_maybe will_set_parameter_int
    will_set_parameter_int_always will_set_parameter_int_count
    will_set_parameter_int_maybe will_set_parameter_maybe
    will_set_parameter_ptr will_set_parameter_ptr_always
    will_set_parameter_ptr_count will_set_parameter_ptr_maybe
    will_set_parameter_ptr_type will_set_parameter_uint
    will_set_parameter_uint_always will_set_parameter_uint_count
    will_set_parameter_uint_maybe
    """.split()
)
NAME = re.compile(r"\b(" + "|".join(sorted(CMOCKA_2_ONLY)) + r")\b")


def compat_names() -> set[str]:
    """The names cmocka-compat.h defines under #ifndef NAME."""
    if not COMPAT.exists():
        return set()
    return set(re.findall(r"^\s*#\s*ifndef\s+(\w+)", COMPAT.read_text(), re.M))


class CmockaCompatTest(unittest.TestCase):
    def test_no_cmocka_2_only_names(self):
        mapped = compat_names()
        found = []
        for path in sorted(CMOCKA.rglob("*")):
            if path.suffix not in {".c", ".cpp", ".h", ".hpp"} or path == COMPAT:
                continue
            source = path.read_text(errors="replace")
            via_compat = COMPAT_INCLUDE.search(source) is not None
            code = COMMENT.sub(lambda m: "\n" * m.group().count("\n"), source)
            for number, line in enumerate(code.splitlines(), 1):
                for name in NAME.findall(line):
                    if not (via_compat and name in mapped):
                        found.append(f"{path.relative_to(ROOT)}:{number}: {name}")
        self.assertEqual(found, [], "cmocka 2.x-only names; map them in cmocka-compat.h")

    def test_compat_maps_only_2_only_names(self):
        self.assertLessEqual(compat_names(), CMOCKA_2_ONLY)


if __name__ == "__main__":
    unittest.main()
