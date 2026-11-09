// The corpus tooling's own specs (profile, eval, expected files, the Azure adapter): run with
// `yarn test:scripts`. `yarn test` covers the package itself, as in the sibling packages.
/** @type {import("../ava-config").AvaConfig} */
const config = {
  files: ["scripts/__test__/*.spec.ts"],
  extensions: ["ts"],
  workerThreads: false,
  environmentVariables: {
    OXC_TSCONFIG_PATH: "./__test__/tsconfig.json"
  },
  nodeArguments: ["--import", "@oxc-node/core/register"]
};

export default config;
