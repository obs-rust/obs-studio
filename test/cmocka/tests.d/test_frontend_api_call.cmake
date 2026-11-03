# obs-websocket's CreateProfile and RemoveProfile returned before the profile
# list had changed (obsproject/obs-websocket#1283): the frontend API's profile
# calls did not wait for the UI thread. Builds the helper the API uses
# (frontend/utility/frontend-api-call.hpp) against QtCore only; no display.
find_package(Qt6 QUIET COMPONENTS Core)
set(HAVE_QT6_CORE FALSE)
if(TARGET Qt6::Core)
  set(HAVE_QT6_CORE TRUE)
endif()
# Target, timeout and Qt runtime contract: ../build/test-graph.json (canonical).
