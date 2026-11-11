# Characterization of the hotkey key-name translations in
# libobs/obs-hotkey-translations.c before their port. It links the real
# libobs and goes through the public API only (obs_hotkeys_set_translations_s,
# obs_key_to_str, obs_key_combination_to_str), so it also covers the hybrid
# libobs. Linux and FreeBSD only: there obs_key_to_str shows the stored name
# of every key the translations name.
# obs_startup needs an X display on Linux.
# Defined in ../build/test-graph.d.
