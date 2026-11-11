# Routed dispatch is used by the live-library pair-latch test. The standalone
# *_steps_rust fixtures select only their own shims (obs-hotkey uses 256 release
# codegen units), so they must not pull this registry/global-dependent adapter
# into a fixture that has no libobs core. Its real source is in libobs.core.
