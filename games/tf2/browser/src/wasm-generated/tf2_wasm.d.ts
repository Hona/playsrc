/* tslint:disable */
/* eslint-disable */

export function initThreadPool(num_threads: number): Promise<any>;

export class wbg_rayon_PoolBuilder {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    build(): void;
    numThreads(): number;
    receiver(): number;
}

export function wbg_rayon_start_worker(receiver: number): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly playsrc_result_copy: (a: number, b: number, c: number) => number;
    readonly playsrc_dispose: (a: number) => number;
    readonly playsrc_game_advance: (a: number, b: number, c: number, d: number) => number;
    readonly playsrc_alloc: (a: number) => number;
    readonly playsrc_compile_map: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly playsrc_compile_map_cached: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly playsrc_compile_metric_milliseconds: (a: number, b: number) => number;
    readonly playsrc_coverage_copy: (a: number, b: number, c: number) => number;
    readonly playsrc_coverage_length: (a: number) => number;
    readonly playsrc_free: (a: number, b: number) => void;
    readonly playsrc_jump_configure: (a: number, b: number, c: number) => number;
    readonly playsrc_model_output_copy: (a: number, b: number, c: number) => number;
    readonly playsrc_model_output_length: (a: number) => number;
    readonly playsrc_model_transact: (a: number, b: number, c: number) => number;
    readonly playsrc_particle_output_copy: (a: number, b: number, c: number) => number;
    readonly playsrc_particle_output_length: (a: number) => number;
    readonly playsrc_particle_transact: (a: number, b: number, c: number) => number;
    readonly playsrc_presentation_copy: (a: number, b: number, c: number) => number;
    readonly playsrc_presentation_length: (a: number) => number;
    readonly playsrc_presentation_release: (a: number) => number;
    readonly playsrc_resource_copy: (a: number, b: number) => number;
    readonly playsrc_resource_decode: (a: number, b: number) => number;
    readonly playsrc_resource_length: () => number;
    readonly playsrc_result_derived_hash: (a: number, b: number) => number;
    readonly playsrc_result_error: (a: number) => number;
    readonly playsrc_result_hash: (a: number, b: number) => number;
    readonly playsrc_result_length: (a: number) => number;
    readonly playsrc_runtime_count: (a: number, b: number) => number;
    readonly playsrc_simulation_error: () => number;
    readonly playsrc_simulation_error_copy: (a: number, b: number) => number;
    readonly playsrc_simulation_error_length: () => number;
    readonly playsrc_simulation_observe: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly playsrc_simulation_output_copy: (a: number, b: number, c: number) => number;
    readonly playsrc_simulation_output_length: (a: number) => number;
    readonly playsrc_snapshot_copy: (a: number, b: number, c: number) => number;
    readonly playsrc_snapshot_length: (a: number) => number;
    readonly playsrc_spawn_copy: (a: number, b: number, c: number) => number;
    readonly playsrc_teleport_count: (a: number) => number;
    readonly playsrc_teleport_destination_count: (a: number) => number;
    readonly playsrc_visibility_output_copy: (a: number, b: number, c: number) => number;
    readonly playsrc_visibility_output_length: (a: number) => number;
    readonly playsrc_visibility_query: (a: number, b: number) => number;
    readonly __wbg_wbg_rayon_poolbuilder_free: (a: number, b: number) => void;
    readonly initThreadPool: (a: number) => any;
    readonly wbg_rayon_poolbuilder_build: (a: number) => void;
    readonly wbg_rayon_poolbuilder_numThreads: (a: number) => number;
    readonly wbg_rayon_poolbuilder_receiver: (a: number) => number;
    readonly wbg_rayon_start_worker: (a: number) => void;
    readonly memory: WebAssembly.Memory;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_thread_destroy: (a?: number, b?: number, c?: number) => void;
    readonly __wbindgen_start: (a: number) => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput, memory?: WebAssembly.Memory, thread_stack_size?: number }} module - Passing `SyncInitInput` directly is deprecated.
 * @param {WebAssembly.Memory} memory - Deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput, memory?: WebAssembly.Memory, thread_stack_size?: number } | SyncInitInput, memory?: WebAssembly.Memory): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput>, memory?: WebAssembly.Memory, thread_stack_size?: number }} module_or_path - Passing `InitInput` directly is deprecated.
 * @param {WebAssembly.Memory} memory - Deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path: { module_or_path: InitInput | Promise<InitInput>, memory?: WebAssembly.Memory, thread_stack_size?: number } | InitInput | Promise<InitInput>, memory?: WebAssembly.Memory): Promise<InitOutput>;
