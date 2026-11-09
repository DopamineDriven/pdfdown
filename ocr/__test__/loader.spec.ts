import { spawnSync } from "child_process";

import test from "ava";

// The OCR package loader bounds Tesseract's OpenMP pool before libtesseract loads (see ocr/index.js).
// Each case runs in a fresh node process so the environment is observed exactly as the loader leaves it.
const ompAfterRequire = (env: NodeJS.ProcessEnv) => {
  const clean = Object.fromEntries(Object.entries(env).filter(([k]) => !k.startsWith("OMP_")));
  const run = spawnSync(
    process.execPath,
    ["-e", "require('./ocr/index.js'); process.stdout.write(process.env.OMP_THREAD_LIMIT ?? 'unset')"],
    { cwd: process.cwd(), env: { ...clean, ...pick(env) }, encoding: "utf8" }
  );
  if (run.status !== 0) throw new Error(run.stderr);
  return run.stdout;
};

const pick = (env: NodeJS.ProcessEnv) => Object.fromEntries(Object.entries(env).filter(([k]) => k.startsWith("OMP_")));

test("loader: OMP_THREAD_LIMIT defaults to 1 so OpenMP cannot oversubscribe the OCR workers", (t) => {
  t.is(ompAfterRequire({ ...process.env, OMP_THREAD_LIMIT: undefined, OMP_NUM_THREADS: undefined }), "1");
});

test("loader: a user-set OMP_THREAD_LIMIT is respected", (t) => {
  t.is(ompAfterRequire({ ...process.env, OMP_THREAD_LIMIT: "3", OMP_NUM_THREADS: undefined }), "3");
});

test("loader: a user-set OMP_NUM_THREADS is respected and OMP_THREAD_LIMIT stays unset", (t) => {
  t.is(ompAfterRequire({ ...process.env, OMP_THREAD_LIMIT: undefined, OMP_NUM_THREADS: "2" }), "unset");
});
