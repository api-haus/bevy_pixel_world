// Web Worker for OPFS persistence I/O
// Runs in a separate thread, can use FileSystemSyncAccessHandle

// Worker state
let rootDir = null;
let saveFile = null;
let syncHandle = null;
let chunkIndex = new Map(); // Map<string, {offset, size, storageType}>
let bodyIndex = new Map();  // Map<string, {offset, size, chunkPos}>
let dataWritePos = 0;
let worldSeed = 0;

// Header constants (must match Rust)
const HEADER_SIZE = 64;
const PAGE_TABLE_ENTRY_SIZE = 24;
const BODY_INDEX_ENTRY_SIZE = 28;
const ENTITY_HEADER_SIZE = 8;
const MAX_CHUNK_SIZE = 100_000_000; // 100MB sanity limit for corrupt entry detection

// Message handler
self.onmessage = async (event) => {
	const { type, ...data } = event.data;

	try {
		let result;
		switch (type) {
			case 'Initialize':
				result = await handleInitialize(data.saveName, data.seed);
				break;
			case 'LoadChunk':
				result = await handleLoadChunk(data.chunkX, data.chunkY);
				break;
			case 'WriteChunk':
				result = await handleWriteChunk(data.chunkX, data.chunkY, data.data);
				break;
			case 'SaveBody':
				result = await handleSaveBody(data.stableId, data.data);
				break;
			case 'RemoveBody':
				result = await handleRemoveBody(data.stableId);
				break;
			case 'Flush':
				result = await handleFlush();
				break;
			case 'Shutdown':
				await handleFlush();
				if (syncHandle) {
					syncHandle.close();
					syncHandle = null;
				}
				result = { type: 'FlushComplete' };
				break;
			case 'DeleteSave':
				result = await handleDeleteSave();
				break;
			default:
				result = { type: 'Error', message: `Unknown command: ${type}` };
		}
		self.postMessage(result);
	} catch (e) {
		self.postMessage({ type: 'Error', message: e.message || String(e) });
	}
};

async function handleInitialize(saveName, seed) {
	// Get OPFS root
	rootDir = await navigator.storage.getDirectory();

	const fileName = `${saveName}.save`;

	// Try to open existing file or create new one
	let fileExists = false;
	try {
		saveFile = await rootDir.getFileHandle(fileName, { create: false });
		fileExists = true;
	} catch (e) {
		if (e.name === 'NotFoundError') {
			saveFile = await rootDir.getFileHandle(fileName, { create: true });
		} else {
			throw e;
		}
	}

	// Get sync access handle - this is why we need a Web Worker!
	syncHandle = await saveFile.createSyncAccessHandle();

	if (fileExists && syncHandle.getSize() >= HEADER_SIZE) {
		// Read existing file
		await readExistingFile();
	} else {
		// Create new file with header
		await createNewFile(seed);
	}

	return {
		type: 'Initialized',
		chunkCount: chunkIndex.size,
		bodyCount: bodyIndex.size,
		worldSeed: worldSeed
	};
}

async function createNewFile(seed) {
	worldSeed = seed;

	// Create header buffer
	const header = new ArrayBuffer(HEADER_SIZE);
	const view = new DataView(header);

	// Magic bytes "PXSV"
	view.setUint8(0, 0x50); // P
	view.setUint8(1, 0x58); // X
	view.setUint8(2, 0x53); // S
	view.setUint8(3, 0x56); // V

	// Version (1)
	view.setUint32(4, 1, true);

	// World seed (u64 as two u32s)
	view.setUint32(8, Number(BigInt(seed) & BigInt(0xFFFFFFFF)), true);
	view.setUint32(12, Number(BigInt(seed) >> BigInt(32)), true);

	// Timestamps (created, modified)
	const now = Math.floor(Date.now() / 1000);
	view.setBigUint64(16, BigInt(now), true);
	view.setBigUint64(24, BigInt(now), true);

	// Chunk count (0)
	view.setUint32(32, 0, true);

	// Data region pointer (HEADER_SIZE)
	view.setBigUint64(40, BigInt(HEADER_SIZE), true);

	// Page table size (0)
	view.setUint32(48, 0, true);

	// Entity section pointer (0 = no entities)
	view.setBigUint64(52, BigInt(0), true);

	// Reserved bytes 60-63 already 0

	// Write header
	syncHandle.write(new Uint8Array(header), { at: 0 });
	syncHandle.flush();

	dataWritePos = HEADER_SIZE;
	chunkIndex.clear();
	bodyIndex.clear();
}

async function readExistingFile() {
	// Read header
	const headerBuf = new ArrayBuffer(HEADER_SIZE);
	syncHandle.read(new Uint8Array(headerBuf), { at: 0 });
	const view = new DataView(headerBuf);

	// Verify magic
	const magic = String.fromCharCode(
		view.getUint8(0), view.getUint8(1), view.getUint8(2), view.getUint8(3)
	);
	if (magic !== 'PXSV') {
		throw new Error(`Invalid save file magic: ${magic}`);
	}

	// Read world seed
	const seedLow = view.getUint32(8, true);
	const seedHigh = view.getUint32(12, true);
	worldSeed = Number(BigInt(seedHigh) << BigInt(32) | BigInt(seedLow));

	// Read chunk count
	const chunkCount = view.getUint32(32, true);

	// Read data region pointer (this is where page table is)
	const dataRegionPtr = Number(view.getBigUint64(40, true));

	// Read entity section pointer
	const entitySectionPtr = Number(view.getBigUint64(52, true));

	// Read page table
	chunkIndex.clear();
	if (chunkCount > 0) {
		const pageTableSize = chunkCount * PAGE_TABLE_ENTRY_SIZE;
		const pageTableBuf = new ArrayBuffer(pageTableSize);
		syncHandle.read(new Uint8Array(pageTableBuf), { at: dataRegionPtr });

		for (let i = 0; i < chunkCount; i++) {
			const entryView = new DataView(pageTableBuf, i * PAGE_TABLE_ENTRY_SIZE, PAGE_TABLE_ENTRY_SIZE);
			const chunkX = entryView.getInt32(0, true);
			const chunkY = entryView.getInt32(4, true);
			const offset = Number(entryView.getBigUint64(8, true));
			const size = entryView.getUint32(16, true);
			const storageType = entryView.getUint8(20);

			// Skip corrupt entries
			if (size === 0 || size > MAX_CHUNK_SIZE) {
				console.warn(`[Worker] Skipping corrupt page table entry at ${chunkX},${chunkY}: size=${size}`);
				continue;
			}

			const key = `${chunkX},${chunkY}`;
			chunkIndex.set(key, { offset, size, storageType });
		}
	}

	// Read entity section
	bodyIndex.clear();
	if (entitySectionPtr > 0) {
		// Read entity header
		const entityHeaderBuf = new ArrayBuffer(ENTITY_HEADER_SIZE);
		syncHandle.read(new Uint8Array(entityHeaderBuf), { at: entitySectionPtr });
		const entityView = new DataView(entityHeaderBuf);
		const entityCount = entityView.getUint32(0, true);

		if (entityCount > 0) {
			const bodyIndexSize = entityCount * BODY_INDEX_ENTRY_SIZE;
			const bodyIndexBuf = new ArrayBuffer(bodyIndexSize);
			syncHandle.read(new Uint8Array(bodyIndexBuf), { at: entitySectionPtr + ENTITY_HEADER_SIZE });

			for (let i = 0; i < entityCount; i++) {
				const entryView = new DataView(bodyIndexBuf, i * BODY_INDEX_ENTRY_SIZE, BODY_INDEX_ENTRY_SIZE);
				const stableIdLow = entryView.getUint32(0, true);
				const stableIdHigh = entryView.getUint32(4, true);
				const stableId = Number(BigInt(stableIdHigh) << BigInt(32) | BigInt(stableIdLow));
				const offset = Number(entryView.getBigUint64(8, true));
				const size = entryView.getUint32(16, true);
				const chunkX = entryView.getInt32(20, true);
				const chunkY = entryView.getInt32(24, true);

				bodyIndex.set(String(stableId), { offset, size, chunkPos: { x: chunkX, y: chunkY } });
			}
		}
	}

	// Data write position is at the page table start
	dataWritePos = dataRegionPtr;
}

function handleLoadChunk(chunkX, chunkY) {
	const key = `${chunkX},${chunkY}`;
	const entry = chunkIndex.get(key);

	console.log(`[Worker] LoadChunk ${key}: entry=${entry ? `offset=${entry.offset}, size=${entry.size}` : 'null'}, index size=${chunkIndex.size}`);

	if (!entry) {
		return {
			type: 'ChunkLoaded',
			chunkX,
			chunkY,
			data: null
		};
	}

	// Validate size to prevent corrupt entries from crashing
	if (entry.size === 0 || entry.size > MAX_CHUNK_SIZE) {
		console.warn(`[Worker] Corrupt chunk entry ${key}: size=${entry.size}, treating as missing`);
		chunkIndex.delete(key);
		return { type: 'ChunkLoaded', chunkX, chunkY, data: null };
	}

	// Read chunk data
	const data = new Uint8Array(entry.size);
	syncHandle.read(data, { at: entry.offset });

	return {
		type: 'ChunkLoaded',
		chunkX,
		chunkY,
		data,
		storageType: entry.storageType,
		seederNeeded: entry.storageType === 1 // Delta = 1
	};
}

function handleWriteChunk(chunkX, chunkY, data) {
	const key = `${chunkX},${chunkY}`;

	console.log(`[Worker] WriteChunk ${key}: size=${data.length}, writePos=${dataWritePos}`);

	// Write size prefix + data
	const sizeBuf = new ArrayBuffer(4);
	new DataView(sizeBuf).setUint32(0, data.length, true);

	syncHandle.write(new Uint8Array(sizeBuf), { at: dataWritePos });
	syncHandle.write(data, { at: dataWritePos + 4 });

	// Update index
	chunkIndex.set(key, {
		offset: dataWritePos + 4, // Skip size prefix
		size: data.length,
		storageType: 2 // Full = 2
	});

	dataWritePos += 4 + data.length;

	console.log(`[Worker] WriteChunk ${key} complete, index size=${chunkIndex.size}`);

	return {
		type: 'WriteComplete',
		chunkX,
		chunkY
	};
}

function handleSaveBody(stableId, data) {
	// Write data
	syncHandle.write(data, { at: dataWritePos });

	// Parse chunk pos from data (first 8 bytes after stable_id are i32 chunk_x, i32 chunk_y)
	// Actually, let's read it from a fixed offset in the record format
	// For simplicity, store at origin - the Rust side will handle proper indexing
	const chunkX = 0;
	const chunkY = 0;

	// Update index
	bodyIndex.set(String(stableId), {
		offset: dataWritePos,
		size: data.length,
		chunkPos: { x: chunkX, y: chunkY }
	});

	dataWritePos += data.length;

	return {
		type: 'BodySaveComplete',
		stableId
	};
}

function handleRemoveBody(stableId) {
	bodyIndex.delete(String(stableId));

	return {
		type: 'BodyRemoveComplete',
		stableId
	};
}

async function handleDeleteSave() {
	// Close current handle
	if (syncHandle) {
		syncHandle.close();
		syncHandle = null;
	}

	// Delete the save file
	if (saveFile && rootDir) {
		const fileName = saveFile.name;
		await rootDir.removeEntry(fileName);
		console.log(`[Worker] Deleted save file: ${fileName}`);
	}

	// Clear in-memory state
	chunkIndex.clear();
	bodyIndex.clear();
	dataWritePos = HEADER_SIZE;
	worldSeed = 0;

	// Reinitialize with a fresh empty file
	saveFile = await rootDir.getFileHandle('world.save', { create: true });
	syncHandle = await saveFile.createSyncAccessHandle();
	await createNewFile(Date.now()); // Use timestamp as new seed

	return { type: 'DeleteComplete' };
}

async function handleFlush() {
	// Update header with current counts and pointers
	const header = new ArrayBuffer(HEADER_SIZE);
	const view = new DataView(header);

	// Magic bytes "PXSV"
	view.setUint8(0, 0x50);
	view.setUint8(1, 0x58);
	view.setUint8(2, 0x53);
	view.setUint8(3, 0x56);

	// Version
	view.setUint32(4, 1, true);

	// World seed
	view.setUint32(8, Number(BigInt(worldSeed) & BigInt(0xFFFFFFFF)), true);
	view.setUint32(12, Number(BigInt(worldSeed) >> BigInt(32)), true);

	// Timestamps
	const now = Math.floor(Date.now() / 1000);
	// Keep created time by reading it first
	const existingHeader = new ArrayBuffer(HEADER_SIZE);
	syncHandle.read(new Uint8Array(existingHeader), { at: 0 });
	const existingView = new DataView(existingHeader);
	view.setBigUint64(16, existingView.getBigUint64(16, true), true); // created
	view.setBigUint64(24, BigInt(now), true); // modified

	// Chunk count
	view.setUint32(32, chunkIndex.size, true);

	// Data region pointer (where page table goes)
	view.setBigUint64(40, BigInt(dataWritePos), true);

	// Page table size
	const pageTableSize = chunkIndex.size * PAGE_TABLE_ENTRY_SIZE;
	view.setUint32(48, pageTableSize, true);

	// Write page table
	if (chunkIndex.size > 0) {
		const pageTableBuf = new ArrayBuffer(pageTableSize);
		let i = 0;
		for (const [key, entry] of chunkIndex) {
			const [x, y] = key.split(',').map(Number);
			const entryView = new DataView(pageTableBuf, i * PAGE_TABLE_ENTRY_SIZE, PAGE_TABLE_ENTRY_SIZE);
			entryView.setInt32(0, x, true);
			entryView.setInt32(4, y, true);
			entryView.setBigUint64(8, BigInt(entry.offset), true);
			entryView.setUint32(16, entry.size, true);
			entryView.setUint8(20, entry.storageType);
			// 3 padding bytes already 0
			i++;
		}
		syncHandle.write(new Uint8Array(pageTableBuf), { at: dataWritePos });
	}

	// Entity section
	const entitySectionPtr = dataWritePos + pageTableSize;
	if (bodyIndex.size > 0) {
		view.setBigUint64(52, BigInt(entitySectionPtr), true);

		// Write entity header
		const entityHeaderBuf = new ArrayBuffer(ENTITY_HEADER_SIZE);
		const entityHeaderView = new DataView(entityHeaderBuf);
		entityHeaderView.setUint32(0, bodyIndex.size, true);
		syncHandle.write(new Uint8Array(entityHeaderBuf), { at: entitySectionPtr });

		// Write body index
		const bodyIndexSize = bodyIndex.size * BODY_INDEX_ENTRY_SIZE;
		const bodyIndexBuf = new ArrayBuffer(bodyIndexSize);
		let j = 0;
		for (const [idStr, entry] of bodyIndex) {
			const stableId = BigInt(idStr);
			const entryView = new DataView(bodyIndexBuf, j * BODY_INDEX_ENTRY_SIZE, BODY_INDEX_ENTRY_SIZE);
			entryView.setUint32(0, Number(stableId & BigInt(0xFFFFFFFF)), true);
			entryView.setUint32(4, Number(stableId >> BigInt(32)), true);
			entryView.setBigUint64(8, BigInt(entry.offset), true);
			entryView.setUint32(16, entry.size, true);
			entryView.setInt32(20, entry.chunkPos.x, true);
			entryView.setInt32(24, entry.chunkPos.y, true);
			j++;
		}
		syncHandle.write(new Uint8Array(bodyIndexBuf), { at: entitySectionPtr + ENTITY_HEADER_SIZE });
	} else {
		view.setBigUint64(52, BigInt(0), true);
	}

	// Write header
	syncHandle.write(new Uint8Array(header), { at: 0 });

	// Flush to disk
	syncHandle.flush();

	return { type: 'FlushComplete' };
}
