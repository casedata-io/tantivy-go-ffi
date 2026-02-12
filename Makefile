.PHONY: build build-lib install uninstall example clean

# Detect current platform
UNAME_S := $(shell uname -s)
UNAME_M := $(shell uname -m)

ifeq ($(UNAME_S),Darwin)
  ifeq ($(UNAME_M),arm64)
    LIB_DIR = libs/darwin_arm64
  else
    LIB_DIR = libs/darwin_amd64
  endif
else
  ifeq ($(UNAME_M),aarch64)
    LIB_DIR = libs/linux_arm64
  else
    LIB_DIR = libs/linux_amd64
  endif
endif

# Build the Rust native library and copy to libs/
build:
	cargo build --release
	mkdir -p $(LIB_DIR)
	cp target/release/libtantivy_go.a $(LIB_DIR)/

# Build only (alias for CI — just build, skip copy)
build-lib:
	cargo build --release

# Install system-wide (optional, not needed if using pre-built libs)
install: build
	sudo mkdir -p /usr/local/lib /usr/local/include
	sudo cp $(LIB_DIR)/libtantivy_go.a /usr/local/lib/
	sudo cp tantivy_go.h /usr/local/include/
	@echo ""
	@echo "✅ Installed to /usr/local/lib/ and /usr/local/include/"

# Uninstall
uninstall:
	sudo rm -f /usr/local/lib/libtantivy_go.a
	sudo rm -f /usr/local/include/tantivy_go.h
	@echo "✅ Uninstalled."

# Run the example (uses pre-built libs, no Rust needed)
example:
	cd example && go run .

# Clean build artifacts
clean:
	cargo clean 2>/dev/null || true
	rm -rf /tmp/tantivy-example-*
