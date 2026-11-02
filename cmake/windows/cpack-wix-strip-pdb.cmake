# OBS CMake Windows CPack pre-build script: drop PDBs from the MSI stage (#3476)
#
# Runs after CPack staged the install tree and before WiX packs it.

file(GLOB_RECURSE _obs_msi_pdbs "${CPACK_TEMPORARY_DIRECTORY}/*.pdb")
if(_obs_msi_pdbs)
  file(REMOVE ${_obs_msi_pdbs})
endif()
