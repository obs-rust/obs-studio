#include "frontend/build/frontend-fallback-warning.pch.hpp"

/* RED (#2218): nothing is logged, shown or called back yet. */

namespace FallbackWarning {

void SetMainWindow(QWidget *, const Texts &) {}

void Warn(const char *, const char *, const char *, bool, Callback, void *) {}

} // namespace FallbackWarning
