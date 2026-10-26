# A source saved before 33.0 as "Monitor Only" must stay out of the program
# mix (#2444): it loads as monitoring on and muted, Monitor and Output as on
# and unmuted, off as off, and a source saved by 33.0 loads as saved. The
# monitor feed must carry the audio in every case. Mixing needs core video,
# so it runs (under xvfb-run) on Linux and skips without a display. libobs
# finds its effect files relative to the working directory.
# Defined in ../build/test-graph.json.
