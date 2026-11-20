# The test build must not contract a multiply and an add into an FMA (#3422):
# no compile option of its own, so it fails when the build-wide
# -ffp-contract=off of ENABLE_UNIT_TESTS is missing on a host that contracts.
# Target and test are declared in ../build/test-graph.json.
