export class EcoSim {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        EcoSimFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_ecosim_free(ptr, 0);
    }
    /**
     * @returns {bigint}
     */
    day() {
        const ret = wasm.ecosim_day(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * @returns {string}
     */
    history_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.ecosim_history_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Inject `count` immigrants now (from seeds if set, else random).
     * @param {number} count
     */
    immigrate(count) {
        wasm.ecosim_immigrate(this.__wbg_ptr, count);
    }
    /**
     * @param {string} planet_json
     * @param {number} grid
     * @param {bigint} seed
     */
    constructor(planet_json, grid, seed) {
        const val2 = +grid;
        const val3 = BigInt.asIntN(64, seed);
        const ptr0 = passStringToWasm0(planet_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.ecosim_new(ptr0, len0, val2, val3);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0];
        EcoSimFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @returns {number}
     */
    pending_evals() {
        const ret = wasm.ecosim_pending_evals(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Largest plant of a lineage, decoded for display: JSON {genome, body}, or "".
     * @param {number} lineage
     * @returns {string}
     */
    plant_representative_json(lineage) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.ecosim_plant_representative_json(this.__wbg_ptr, lineage);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Representative genome of an animal lineage as JSON {genome, env}, or "".
     * @param {number} lineage
     * @returns {string}
     */
    representative_json(lineage) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.ecosim_representative_json(this.__wbg_ptr, lineage);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Add archive champions (JSON array of genomes) as immigrant seed stock.
     * @param {string} genomes_json
     */
    set_seeds(genomes_json) {
        const ptr0 = passStringToWasm0(genomes_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.ecosim_set_seeds(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @returns {string}
     */
    snapshot_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.ecosim_snapshot_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Advance `days` ticks, running at most `evals` physics trait measurements first.
     * @param {number} days
     * @param {number} evals
     */
    tick(days, evals) {
        wasm.ecosim_tick(this.__wbg_ptr, days, evals);
    }
}
if (Symbol.dispose) EcoSim.prototype[Symbol.dispose] = EcoSim.prototype.free;

export class EvoWorld {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        EvoWorldFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_evoworld_free(ptr, 0);
    }
    /**
     * All archive elite genomes as a JSON array (seed stock for the ecology).
     * @returns {string}
     */
    elites_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.evoworld_elites_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Insert the completed generation; returns the summary as JSON or "".
     * @param {number} elapsed_s
     * @returns {string}
     */
    finish(elapsed_s) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.evoworld_finish(this.__wbg_ptr, elapsed_s);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {number}
     */
    generation() {
        const ret = wasm.evoworld_generation(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {string}
     */
    grid_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.evoworld_grid_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {string} planet_json
     * @param {string} env
     * @param {number} episode_time
     * @param {number} batch
     * @param {bigint} seed
     */
    constructor(planet_json, env, episode_time, batch, seed) {
        const val3 = +episode_time;
        const val4 = +batch;
        const val5 = BigInt.asIntN(64, seed);
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(planet_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(env, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.evoworld_new(ptr0, len0, ptr1, len1, val3, val4, val5);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0];
        EvoWorldFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Evaluate up to `max_evals` creatures. Returns true when a generation
     * completed (call `finish` to insert it).
     * @param {number} max_evals
     * @returns {boolean}
     */
    pump(max_evals) {
        const ret = wasm.evoworld_pump(this.__wbg_ptr, max_evals);
        return ret !== 0;
    }
    /**
     * Champion as JSON if it improved since the last call, else "".
     * @returns {string}
     */
    take_new_champion_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.evoworld_take_new_champion_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) EvoWorld.prototype[Symbol.dispose] = EvoWorld.prototype.free;

export class ReplaySim {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        ReplaySimFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_replaysim_free(ptr, 0);
    }
    /**
     * @returns {boolean}
     */
    airborne() {
        const ret = wasm.replaysim_airborne(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {number}
     */
    base_freq() {
        const ret = wasm.replaysim_base_freq(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Float32Array}
     */
    com() {
        const ret = wasm.replaysim_com(this.__wbg_ptr);
        var v1 = getArrayF32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * @returns {number}
     */
    distance() {
        const ret = wasm.replaysim_distance(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {boolean}
     */
    exploded() {
        const ret = wasm.replaysim_exploded(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {number}
     */
    fatigue() {
        const ret = wasm.replaysim_fatigue(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {boolean}
     */
    finished() {
        const ret = wasm.replaysim_finished(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {number}
     */
    ground_y() {
        const ret = wasm.replaysim_ground_y(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    hydration() {
        const ret = wasm.replaysim_hydration(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    lung_frac() {
        const ret = wasm.replaysim_lung_frac(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    membrane_area() {
        const ret = wasm.replaysim_membrane_area(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    membrane_count() {
        const ret = wasm.replaysim_membrane_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @param {string} genome_json
     * @param {string} planet_json
     * @param {string} env
     * @param {number} episode_time
     */
    constructor(genome_json, planet_json, env, episode_time) {
        const val4 = +episode_time;
        let ptr0, len0, ptr1, len1, ptr2, len2;
        try {
            ptr0 = passStringToWasm0(genome_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(planet_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
            ptr2 = passStringToWasm0(env, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len2 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr2) { wasm.__wbindgen_free(ptr2, len2 * 1, 1); }
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.replaysim_new(ptr0, len0, ptr1, len1, ptr2, len2, val4);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0];
        ReplaySimFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @returns {number}
     */
    part_count() {
        const ret = wasm.replaysim_part_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {string}
     */
    parts_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.replaysim_parts_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} n
     */
    step(n) {
        wasm.replaysim_step(this.__wbg_ptr, n);
    }
    /**
     * @returns {number}
     */
    time() {
        const ret = wasm.replaysim_time(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    total_mass() {
        const ret = wasm.replaysim_total_mass(this.__wbg_ptr);
        return ret;
    }
    /**
     * 7 floats per part: x y z qx qy qz qw.
     * @returns {Float32Array}
     */
    transforms() {
        const ret = wasm.replaysim_transforms(this.__wbg_ptr);
        var v1 = getArrayF32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * @returns {number}
     */
    water_level() {
        const ret = wasm.replaysim_water_level(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    water_store() {
        const ret = wasm.replaysim_water_store(this.__wbg_ptr);
        return ret;
    }
}
if (Symbol.dispose) ReplaySim.prototype[Symbol.dispose] = ReplaySim.prototype.free;

/**
 * @param {string} planet_json
 * @returns {string}
 */
export function derive_json(planet_json) {
    let deferred2_0;
    let deferred2_1;
    try {
        const ptr0 = passStringToWasm0(planet_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.derive_json(ptr0, len0);
        deferred2_0 = ret[0];
        deferred2_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
    }
}

/**
 * @returns {number}
 */
export function grid_size() {
    const ret = wasm.grid_size();
    return ret >>> 0;
}

/**
 * Decode any genome as a plant body for display.
 * @param {string} genome_json
 * @param {number} gravity
 * @returns {string}
 */
export function plant_body_json(genome_json, gravity) {
    let deferred3_0;
    let deferred3_1;
    try {
        const val2 = +gravity;
        const ptr0 = passStringToWasm0(genome_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.plant_body_json(ptr0, len0, val2);
        deferred3_0 = ret[0];
        deferred3_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * @returns {string}
 */
export function presets_json() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.presets_json();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}

export function start() {
    wasm.start();
}
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_throw_4c88d8bf264fd984: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg_error_757e9472f8410341: function(arg0, arg1) {
            let deferred0_0;
            let deferred0_1;
            try {
                deferred0_0 = arg0;
                deferred0_1 = arg1;
                console.error(getStringFromWasm0(arg0, arg1));
            } finally {
                wasm.__wbindgen_free(deferred0_0, deferred0_1, 1);
            }
        },
        __wbg_new_bb74001155cb8ca8: function() {
            const ret = new Error();
            return ret;
        },
        __wbg_stack_575c4e0918581b9c: function(arg0, arg1) {
            const ret = arg1.stack;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbindgen_generic_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            table.set(0, undefined);
            table.set(1024, undefined);
            table.set(1025, null);
            table.set(1026, true);
            table.set(1027, false);
        },
    };
    return {
        __proto__: null,
        "./evo_wasm_bg.js": import0,
    };
}

const EcoSimFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_ecosim_free(ptr, 1));
const EvoWorldFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_evoworld_free(ptr, 1));
const ReplaySimFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_replaysim_free(ptr, 1));

function getArrayF32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

let cachedFloat32ArrayMemory0 = null;
function getFloat32ArrayMemory0() {
    if (cachedFloat32ArrayMemory0 === null || cachedFloat32ArrayMemory0.byteLength === 0) {
        cachedFloat32ArrayMemory0 = new Float32Array(wasm.memory.buffer);
    }
    return cachedFloat32ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(bytes) {
    numBytesDecoded += bytes.length;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true });
        cachedTextDecoder.decode();
        numBytesDecoded = bytes.length;
    }
    return cachedTextDecoder.decode(bytes);
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedFloat32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (!module.ok) {
            throw new Error(`failed to fetch Wasm: ${module.status} ${module.statusText} fetching '${module.url}'`);
        }

        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

let __wbg_init_promise;
async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;
    if (__wbg_init_promise === undefined) {
        __wbg_init_promise = __wbg_init_once(module_or_path);
        __wbg_init_promise.catch(() => { __wbg_init_promise = undefined; });
    }
    return __wbg_init_promise;
}

async function __wbg_init_once(module_or_path) {

    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('evo_wasm_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
