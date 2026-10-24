# A source's volume as the mix applies it (#2108): NaN, negative and
# infinite volumes, from the setter or a loaded scene collection, must leave
# the mixed audio finite, and valid volumes must mix as before. Mixing needs
# core video, so it runs (under xvfb-run) on Linux and skips without a
# display. libobs finds its effect files relative to the working directory.
# Defined in ../build/test-graph.json.
