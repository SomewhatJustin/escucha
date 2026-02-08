.PHONY: build test check clippy fmt clean install

# One-step build (Joel Test #2)
build:
	cargo build --release

# Run all tests
test:
	cargo test

# Type check without building
check:
	cargo check

# Lint
clippy:
	cargo clippy -- -D warnings

# Format check
fmt:
	cargo fmt --all -- --check

# Format fix
fmt-fix:
	cargo fmt --all

# Clean build artifacts
clean:
	cargo clean

# Install to ~/.local/bin
install: build
	install -Dm755 target/release/escucha $(HOME)/.local/bin/escucha
	install -Dm644 systemd/escucha.service $(HOME)/.config/systemd/user/escucha.service
	install -Dm644 io.github.escucha.desktop $(HOME)/.local/share/applications/io.github.escucha.desktop
	install -Dm644 assets/io.github.escucha.svg $(HOME)/.local/share/icons/hicolor/scalable/apps/io.github.escucha.svg

# Run all CI checks locally
ci: fmt clippy test
	@echo "All CI checks passed!"
