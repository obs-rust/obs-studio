# Camera registry race: callers and PipeWire definitions occupy separate unity
# groups so GNU ld --wrap intercepts undefined references. Unity and PCH remain on.
# Shared unchanged fixture registration: build/test-graph.d (Refs #1390).
