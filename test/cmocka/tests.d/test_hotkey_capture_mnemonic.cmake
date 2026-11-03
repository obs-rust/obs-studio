# Settings > Hotkeys: Alt+C / Alt+O pressed in a key field pressed the dialog's
# "&Cancel" / "&OK" button instead of being recorded (GNOME and KDE themes,
# obsproject/obs-studio#13385). Builds the frontend helper the key field calls
# and drives a dialog with QTest on Qt's offscreen platform, so it needs Qt
# Widgets and Qt Test but no display and no frontend build.
# Defined in ../build/test-graph.json.
