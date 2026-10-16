# Characterizes media-io/video-scaler-ffmpeg.c output layouts (packed,
# padded, misaligned) and canary integrity ahead of the guarded
# direct-output rework.
add_executable(test_video_scaler test_video_scaler.c)
target_include_directories(test_video_scaler PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_video_scaler PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_video_scaler)
add_test(test_video_scaler ${CMAKE_CURRENT_BINARY_DIR}/test_video_scaler)
