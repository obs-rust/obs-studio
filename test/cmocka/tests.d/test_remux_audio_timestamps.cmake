# Remuxing an MKV to MP4/MOV keeps AAC frames sample accurate (upstream
# obsproject/obs-studio#13249). Generates the MKV with FFmpeg's AAC encoder and
# remuxes it through libobs' media_remux; needs no display.
# Target and runtime contract: ../build/test-graph.d (canonical).
