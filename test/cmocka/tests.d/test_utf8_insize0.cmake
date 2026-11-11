# utf8_to_wchar's insize == 0 (NUL-terminated) path for 3-byte sequences.
# The old sentinel limit (unsigned char *)-1 is not a valid one-past-the-end
# pointer; at -O2 GCC folds the length checks away and the function answers 0
# for U+0800-U+FFFF. These assertions hold the -O0 behaviour in every
# configuration.
# Defined in ../build/test-graph.d.
