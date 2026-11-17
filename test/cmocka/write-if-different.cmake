# Generated test inputs must keep their mtimes when a configure changes nothing.
include_guard(GLOBAL)

function(obs_test_write_file path content)
  # Substitute the content once, without interpreting @names@, ${variables}, or
  # generator expressions inside the extracted C/headers/fixture data.
  file(CONFIGURE OUTPUT "${path}" CONTENT "@content@" @ONLY)
endfunction()
