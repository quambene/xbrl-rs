
# Run tests
test:
    cargo test --features taxonomy-test

# Run unit tests
test-unit:
    cargo test --lib

# Run integration tests
test-integration:
    cargo test --test '*'

# Run taxonomy tests
test-taxonomy:
    cargo test --features taxonomy-test

# Run conformance tests
test-conformance:
    cargo test conformance_suite --features conformance-test
