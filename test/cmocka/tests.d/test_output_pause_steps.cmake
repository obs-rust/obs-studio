# Characterization of the pause plan of libobs/obs-output.c, moved to
# libobs/obs-output-pause.c before its port to rust/obs-output (output_pause,
# #3020): the OBS_OUTPUT_CAN_PAUSE gate, the "already in that state"
# success, the slots whose pause records are locked, the pause time on the
# video frame grid (no frame yet, on the grid, one ns past a frame, a clock
# behind the last frame), whether a record can start or end a pause, its end
# and offset, and whole pauses and unpauses run the way obs-output.c runs
# them: one video and two audio encoders, one record not clear failing the
# whole pause, an encoded output with audio encoders only (it can't pause:
# characterized, not endorsed) and a raw output. The inputs the port changes
# on purpose (one encoder in two slots, a frame interval of 0) are not
# covered. Builds the moved file on its own: no display and no libobs core,
# so it runs on every platform. The functions are hidden in libobs, so this
# characterizes that C source; the test calls only them. The Tier 3 parity
# tests in rust/obs-output compare the shims with this C.
# Target and test are declared in ../build/test-graph.json.
