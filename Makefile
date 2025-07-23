.PHONY: all build test fmt fmt-check clippy clean run help install-deps check release

# Default target
all: fmt clippy test build

# Help target
help:
	@echo "Available targets:"
	@echo "  make all          - Run fmt, clippy, test, and build"
	@echo "  make build        - Build the project"
	@echo "  make test         - Run all tests"
	@echo "  make fmt          - Format code with rustfmt"
	@echo "  make fmt-check    - Check code formatting without modifying"
	@echo "  make clippy       - Run clippy linter with strict rules"
	@echo "  make clean        - Clean build artifacts"
	@echo "  make run          - Run the REPL"
	@echo "  make check        - Quick check (fmt-check + clippy + test)"
	@echo "  make release      - Build optimized release version"
	@echo "  make install-deps - Install system dependencies (macOS/Ubuntu)"

# Install system dependencies
install-deps:
	@echo "Installing system dependencies..."
	@if [ "$$(uname)" = "Darwin" ]; then \
		echo "macOS detected - cpal should work with CoreAudio (no deps needed)"; \
	elif [ "$$(uname)" = "Linux" ]; then \
		echo "Linux detected - installing ALSA development libraries"; \
		sudo apt-get update && sudo apt-get install -y libasound2-dev; \
	else \
		echo "Unsupported OS. Please install audio dependencies manually."; \
	fi

# Build the project
build:
	cargo build --verbose

# Build the project
install-local:
	cargo install --path .

install: build install-local

# Build release version
release:
	cargo build --release --verbose

# Run tests
test:
	cargo test --verbose

# Format code
fmt:
	cargo fmt --all

# Check formatting (CI-compatible)
fmt-check:
	cargo fmt --all -- --check

# Run clippy with strict settings matching CI
clippy:
	cargo clippy --all-targets --all-features -- \
		-D warnings \
		-W clippy::all \
		-W clippy::pedantic \
		-W clippy::nursery \
		-W clippy::cargo

# Quick check for CI compliance
check: fmt-check clippy test

# Clean build artifacts
clean:
	cargo clean

# Run the REPL
run:
	cargo run -- repl

# Run with a specific patch file
run-file:
	@if [ -z "$(FILE)" ]; then \
		echo "Usage: make run-file FILE=path/to/patch.zim"; \
		exit 1; \
	fi
	cargo run -- play $(FILE)

# Watch for changes and rebuild
watch:
	@if command -v cargo-watch >/dev/null 2>&1; then \
		cargo watch -x check -x test -x build; \
	else \
		echo "cargo-watch not installed. Install with: cargo install cargo-watch"; \
		exit 1; \
	fi

# Run benchmarks (if any)
bench:
	cargo bench

# Generate and open documentation
doc:
	cargo doc --no-deps --open

# Update dependencies
update:
	cargo update

# Check for outdated dependencies
outdated:
	@if command -v cargo-outdated >/dev/null 2>&1; then \
		cargo outdated; \
	else \
		echo "cargo-outdated not installed. Install with: cargo install cargo-outdated"; \
		exit 1; \
	fi

# Security audit
audit:
	@if command -v cargo-audit >/dev/null 2>&1; then \
		cargo audit; \
	else \
		echo "cargo-audit not installed. Install with: cargo install cargo-audit"; \
		exit 1; \
	fi

# Full CI simulation
ci: clean fmt-check clippy test build
	@echo "✅ All CI checks passed!"
