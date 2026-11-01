# libobs binds calls to its own exported functions locally (#3220): built with
# -Bsymbolic-functions, libobs.so has no PLT slot (JUMP_SLOT relocation) for
# a function it defines itself. libobs-opengl, the one shipped object that
# defines libobs names, is dlopen'ed RTLD_LOCAL and never interposed.
if(OS_LINUX)
  find_program(LIBOBS_READELF NAMES readelf)
  find_program(LIBOBS_NM NAMES nm)
  if(LIBOBS_READELF AND LIBOBS_NM)
    add_test(
      NAME libobs_local_binding
      COMMAND
        "${CMAKE_COMMAND}" "-DLIBRARY=$<TARGET_FILE:OBS::libobs>" "-DREADELF=${LIBOBS_READELF}" "-DNM=${LIBOBS_NM}" -P
        "${CMAKE_CURRENT_SOURCE_DIR}/check_libobs_local_binding.cmake"
    )
  endif()
endif()
