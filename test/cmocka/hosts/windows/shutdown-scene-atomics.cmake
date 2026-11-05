# MSVC enables the unchanged fixture's <stdatomic.h> only with both settings
# (#3648). Keep these Windows compiler settings out of other hosts' targets.
set_target_properties(test_shutdown_leaked_scenes PROPERTIES C_STANDARD 11 C_STANDARD_REQUIRED ON)
target_compile_options(test_shutdown_leaked_scenes PRIVATE /experimental:c11atomics)
