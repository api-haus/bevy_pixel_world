// sim2d_noise JS Bridge
//
// Loads the Emscripten-compiled FastNoise2 module and provides
// a clean API for noise generation from WASM.
//
// REQUIRES the Emscripten module to be built. No fallbacks.

let module = null;

// Top-level await: Block module from being "ready" until init completes.
try {
	// Compute module URL relative to this script's location
	// Script ends up at: /snippets/sim2d_noise/js/sim2d_noise_bridge.js (via wasm-bindgen)
	// Target is at: /dist/sim2d_noise.js (copied by trunk)
	const moduleUrl = new URL('../../../dist/sim2d_noise.js', import.meta.url).href;
	const { default: createSim2dNoiseModule } = await import(moduleUrl);
	module = await createSim2dNoiseModule();
	console.log('[sim2d_noise] FastNoise2 module initialized');
} catch (e) {
	console.error('[sim2d_noise] FATAL: FastNoise2 module failed to load:', e.message);
	console.error('[sim2d_noise] Build the module with: cd crates/sim2d_noise && make');
	throw new Error('FastNoise2 Emscripten module required but not available');
}

/**
 * Create a noise node from an encoded node tree string.
 * @param {string} encoded - FastNoise2 encoded node tree
 * @returns {number} Handle to noise node
 * @throws {Error} If module not loaded
 */
export function s2d_create(encoded) {
	if (!module) {
		throw new Error('FastNoise2 module not initialized');
	}

	const len = module.lengthBytesUTF8(encoded) + 1;
	const strPtr = module._malloc(len);
	module.stringToUTF8(encoded, strPtr, len);

	const handle = module._s2d_noise_create(strPtr);
	module._free(strPtr);

	if (handle === 0) {
		throw new Error('Failed to create noise node from encoded string');
	}

	return handle;
}

/**
 * Generate 2D noise and return as Float32Array.
 * @param {number} handle - Noise node handle from s2d_create
 * @param {number} xOff - X offset
 * @param {number} yOff - Y offset
 * @param {number} xCnt - X sample count
 * @param {number} yCnt - Y sample count
 * @param {number} xStep - X step size
 * @param {number} yStep - Y step size
 * @param {number} seed - Random seed
 * @returns {Float32Array} Noise values
 * @throws {Error} If module not loaded or handle invalid
 */
export function s2d_gen_2d(handle, xOff, yOff, xCnt, yCnt, xStep, yStep, seed) {
	if (!module) {
		throw new Error('FastNoise2 module not initialized');
	}
	if (handle === 0) {
		throw new Error('Invalid noise node handle');
	}

	const count = xCnt * yCnt;
	const outPtr = module._malloc(count * 4);

	module._s2d_noise_gen_2d(
		handle, outPtr,
		xOff, yOff,
		xCnt, yCnt,
		xStep, yStep,
		seed
	);

	// Copy from WASM heap and free
	const result = new Float32Array(module.HEAPF32.buffer, outPtr, count).slice();
	module._free(outPtr);
	return result;
}

/**
 * Destroy a noise node and free its resources.
 * @param {number} handle - Noise node handle
 */
export function s2d_destroy(handle) {
	if (module && handle) {
		module._s2d_noise_destroy(handle);
	}
}
