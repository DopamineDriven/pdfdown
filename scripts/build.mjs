// Build the native addon and format what napi generates.
//
// Usage: node scripts/build.mjs [--debug] [--ocr] [napi build arguments...]
//
// CI runs `yarn build --target <triple> [--use-napi-cross | -x]` and `yarn build:ocr --target
// <triple>`. Yarn appends those arguments to the end of a script, which in a `napi build && oxfmt`
// chain hands them to the formatter; this wrapper gives every argument that is not its own to
// `napi build` and formats the generated files afterwards.
import { spawnSync } from "node:child_process";
import { delimiter, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const args = process.argv.slice(2);
const take = (flag) => {
  const at = args.indexOf(flag);
  if (at === -1) return false;
  args.splice(at, 1);
  return true;
};
const debug = take("--debug");
const ocr = take("--ocr");

const napiArgs = ["build", "--platform", "--no-const-enum"];
if (!debug) napiArgs.push("--release");
if (ocr) {
  napiArgs.push(
    "--features",
    "ocr,render",
    "--package-json-path",
    "ocr/package.json",
    "--output-dir",
    "ocr",
    "--js",
    "pdfdown_ocr.napi.js"
  );
} else {
  napiArgs.push("--features", "render");
}
const generated = ocr ? ["ocr/index.d.ts"] : ["index.d.ts", "browser.js"];

// `napi` and `oxfmt` are local binaries: resolve them when run outside `yarn` too.
const binDir = join(dirname(fileURLToPath(import.meta.url)), "..", "node_modules", ".bin");
const env = { ...process.env, PATH: `${binDir}${delimiter}${process.env.PATH ?? ""}` };

const run = (command, commandArgs) => {
  const result = spawnSync(command, commandArgs, { stdio: "inherit", shell: process.platform === "win32", env });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
};

run("napi", [...napiArgs, ...args]);
run("oxfmt", ["--write", ...generated]);
