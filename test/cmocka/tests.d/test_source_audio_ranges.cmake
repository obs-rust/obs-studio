# The range of a source's push-to-mute and push-to-talk delays and of its
# audio balance (#2051), through the setters and a loaded scene collection:
# a negative delay loads as 0, the stop time saturates instead of wrapping,
# and the balance is clamped to 0 to 1 (NaN is 0.5), with the valid values
# unchanged. Starts libobs core audio; like the other core audio tests it
# runs on Linux only.
# Defined in ../build/test-graph.json.
