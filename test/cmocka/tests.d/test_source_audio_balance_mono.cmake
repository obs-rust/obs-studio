# The balance and Force Mono gates of process_audio() through the real
# libobs: a stereo source at the output's rate, read back bit for bit
# through an audio capture callback (centred, balance 0.25, Force Mono and
# both). Starts libobs core audio; like the other core audio tests it runs
# on Linux only.
# Defined in ../build/test-graph.json.
