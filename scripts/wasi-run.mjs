// Cargo runner for wasm32-wasip1: runs a bench or test binary in node's WASI, on V8 like a browser.
//
// The binary is first passed through wasm-opt with the passes a browser build usually gets (skip with WASM_OPT=0).
// WASI has no working directory, so relative paths in CRITERION_HOME and RMPX_BENCH_FILES are made absolute here, and
// criterion's results go to target/wasm32-wasip1/criterion unless CRITERION_HOME says otherwise, apart from the
// native ones.
import { execFileSync } from "node:child_process";
import { readFileSync, statSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { argv, cwd, env, exit } from "node:process";
import { WASI } from "node:wasi";

const [wasmPath, ...args] = argv.slice(2);
if (!wasmPath) {
    console.error("usage: wasi-run.mjs <file.wasm> [args...]");
    exit(2);
}

let module = wasmPath;
if (env.WASM_OPT !== "0") {
    module = wasmPath.replace(/\.wasm$/, ".opt.wasm");
    const stale = (() => {
        try {
            return statSync(module).mtimeMs < statSync(wasmPath).mtimeMs;
        } catch {
            return true;
        }
    })();
    if (stale) {
        // prettier-ignore
        execFileSync("wasm-opt", [
            wasmPath, "-o", module,
            "-O3", "--low-memory-unused", "--zero-filled-memory",
            "--strip-debug", "--strip-dwarf", "--strip-producers",
            "--detect-features",
        ], { stdio: "inherit" });
    }
}

const absolute = (paths) =>
    paths
        .split(":")
        .filter((path) => path !== "")
        .map((path) => resolve(cwd(), path))
        .join(":");

const wasmEnv = { ...env };
// <target>/wasm32-wasip1/<profile>/..., whatever the layout below the profile is
const targetDir = resolve(cwd(), wasmPath).match(/^(.*\/wasm32-wasip1)\//)?.[1] ?? dirname(resolve(cwd(), wasmPath));
wasmEnv.CRITERION_HOME = resolve(cwd(), env.CRITERION_HOME ?? `${targetDir}/criterion`);
if (env.RMPX_BENCH_FILES) {
    wasmEnv.RMPX_BENCH_FILES = absolute(env.RMPX_BENCH_FILES);
}

const wasi = new WASI({
    version: "preview1",
    args: [wasmPath, ...args],
    env: wasmEnv,
    preopens: { "/": "/" },
    returnOnExit: true,
});
const { instance } = await WebAssembly.instantiate(readFileSync(module), wasi.getImportObject());
exit(wasi.start(instance));
