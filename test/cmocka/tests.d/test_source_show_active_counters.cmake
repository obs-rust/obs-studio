# obs_source_inc_showing, inc_active, dec_showing and dec_active (#5601): NULL
# changes nothing and logs the caller's name, inc_showing raises show_refs,
# inc_active raises show_refs and activate_refs, the dec pair undoes each and
# never goes below zero, and no other byte is written. Test-owned struct
# obs_source, no core. Registered once in build/test-graph.json, consumed by
# CMake and Cargo.
