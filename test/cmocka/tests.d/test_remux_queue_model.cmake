# Tools > Remux: the queue model told views the insertion row went away (or
# came back) before rowCount() changed, and sent dataChanged ranges one past
# the last column and row. Builds the real utility/RemuxQueueModel.cpp against
# a stand-in for the frontend's precompiled header (remux-queue-stubs/) and
# runs it on Qt's offscreen platform: Qt Widgets, no display, no frontend.
find_package(Qt6 QUIET COMPONENTS Widgets)
set(HAVE_QT6_WIDGETS FALSE)
if(TARGET Qt6::Widgets)
  set(HAVE_QT6_WIDGETS TRUE)
endif()
# Target and runtime contract: ../build/test-graph.d (canonical).
