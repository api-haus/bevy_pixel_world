# sim2d_noise WASM Build - Emscripten
#
# Builds the sim2d_noise crate to wasm32-unknown-emscripten, producing
# a WASM module with C exports that can be called from JavaScript.
#
# Prerequisites:
#   - Emscripten SDK: source ~/emsdk/emsdk_env.sh
#   - Rust target: rustup target add wasm32-unknown-emscripten
#
# Usage:
#   make build   - Build the WASM module (always rebuilds)
#   make         - Build only if sources changed (for trunk hooks)
#   make clean   - Clean build artifacts

.PHONY: build clean check-emsdk

# Output directory for WASM artifacts
DIST_DIR = dist

# Target directory (crate-local since excluded from workspace)
TARGET_DIR = target/wasm32-unknown-emscripten/release

# Source files to track for incremental builds
SOURCES = $(wildcard src/*.rs) Cargo.toml

# Output files
WASM_JS = $(DIST_DIR)/sim2d_noise.js
WASM_BIN = $(DIST_DIR)/sim2d_noise.wasm

# Emscripten linker flags
EMCC_CFLAGS = \
	-s MODULARIZE=1 \
	-s EXPORT_ES6=1 \
	-s EXPORT_NAME=createSim2dNoiseModule \
	-s EXPORTED_FUNCTIONS=_s2d_noise_create,_s2d_noise_gen_2d,_s2d_noise_destroy,_malloc,_free,_main \
	-s EXPORTED_RUNTIME_METHODS=HEAPF32,stringToUTF8,lengthBytesUTF8 \
	-s ALLOW_MEMORY_GROWTH=1 \
	-s ERROR_ON_UNDEFINED_SYMBOLS=0 \
	-O3

# Default target: incremental build (only if needed)
all: $(WASM_JS)

# Check if Emscripten is available
check-emsdk:
	@if ! command -v emcc >/dev/null 2>&1; then \
		echo ""; \
		echo "=== Emscripten not found ==="; \
		echo "Run: source ~/emsdk/emsdk_env.sh"; \
		echo ""; \
		echo "Skipping sim2d_noise WASM build."; \
		echo "Native builds will still work."; \
		echo ""; \
		exit 1; \
	fi

# Incremental build - only rebuild if sources changed
$(WASM_JS): $(SOURCES) | check-emsdk
	@echo "=== Building sim2d_noise WASM ==="
	@mkdir -p $(DIST_DIR)
	EMCC_CFLAGS="$(EMCC_CFLAGS)" cargo build --release --target wasm32-unknown-emscripten --bin sim2d_noise
	@cp $(TARGET_DIR)/sim2d_noise.js $(DIST_DIR)/
	@cp $(TARGET_DIR)/sim2d_noise.wasm $(DIST_DIR)/
	@echo "Build complete: $(DIST_DIR)/"
	@ls -lh $(DIST_DIR)/sim2d_noise.*

# Force rebuild (ignores timestamps)
build: check-emsdk
	@echo "=== Building sim2d_noise WASM (forced) ==="
	@mkdir -p $(DIST_DIR)
	EMCC_CFLAGS="$(EMCC_CFLAGS)" cargo build --release --target wasm32-unknown-emscripten --bin sim2d_noise
	@cp $(TARGET_DIR)/sim2d_noise.js $(DIST_DIR)/
	@cp $(TARGET_DIR)/sim2d_noise.wasm $(DIST_DIR)/
	@echo "Build complete!"
	@ls -lh $(DIST_DIR)/sim2d_noise.*

clean:
	cargo clean
	rm -rf $(DIST_DIR)/
