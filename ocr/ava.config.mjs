/** @type {import("../ava-config").AvaConfig} */
const config = {
  files: ["ocr/__test__/**/*.spec.ts"],
  extensions: ["ts"],
  workerThreads: false,
  environmentVariables: {
    OXC_TSCONFIG_PATH: "./ocr/tsconfig.json"
  },
  nodeArguments: ["--import", "@oxc-node/core/register"]
};

export default config;
