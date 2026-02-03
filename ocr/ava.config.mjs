export default {
  files: ['ocr/__test__/**/*.spec.ts'],
  extensions: { ts: 'module' },
  timeout: '2m',
  workerThreads: false,
  environmentVariables: {
    OXC_TSCONFIG_PATH: './ocr/tsconfig.json',
  },
  nodeArguments: ['--import', '@oxc-node/core/register'],
}
