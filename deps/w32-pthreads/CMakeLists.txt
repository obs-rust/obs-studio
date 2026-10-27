cmake_minimum_required(VERSION 3.28...3.30)

add_library(w32-pthreads SHARED EXCLUDE_FROM_ALL)
add_library(OBS::w32-pthreads ALIAS w32-pthreads)

target_sources(
  w32-pthreads
  PRIVATE implement.h pthread.c pthread.h sched.h semaphore.h w32-pthreads.rc
)

target_compile_definitions(w32-pthreads PRIVATE __CLEANUP_C PTW32_BUILD)

# MSVC /W4 (#2626): vendored pthreads-win32 keeps its own suppressions rather
# than source edits (the #516 vendored-code exemption): C4324 alignment
# padding, C4057 indirection of a volatile LONG64/size_t pair in the MCS lock,
# C4310 a truncating constant cast, C4456 a shadowed local, C4702 the
# unreachable path after its own assert.
target_compile_options(
  w32-pthreads
  PRIVATE
    $<$<COMPILE_LANG_AND_ID:C,MSVC>:/wd4057>
    $<$<COMPILE_LANG_AND_ID:C,MSVC>:/wd4702>
    $<$<COMPILE_LANG_AND_ID:C,MSVC>:/wd4310>
    $<$<COMPILE_LANG_AND_ID:C,MSVC>:/wd4324>
    $<$<COMPILE_LANG_AND_ID:C,MSVC>:/wd4456>
)

target_include_directories(w32-pthreads PUBLIC "$<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}>")

configure_file(cmake/windows/obs-module.rc.in w32-pthreads.rc)

set_target_properties_obs(w32-pthreads PROPERTIES FOLDER deps)

set_property(TARGET w32-pthreads APPEND PROPERTY PUBLIC_HEADER pthread.h sched.h)

target_export(w32-pthreads)
