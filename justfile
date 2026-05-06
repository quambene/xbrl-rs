
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

# Run cargo check with env vars
check:
    cargo check

# Run cargo clippy with env vars
clippy:
    cargo clippy --all-targets --all-features

# Run cargo build with env vars
build:
    cargo build

# Run cargo build with env vars in release mode
build-release:
    cargo build --release
