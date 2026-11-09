/**
 * The shape of an ava 8 configuration, as ava's own loader accepts it (`lib/load-config.js`,
 * `lib/cli.js`, `lib/extensions.js`) and as `@ava/typescript` validates its `typescript` block.
 *
 * ava ships no type for its configuration: its exports cover tests, assertions, macros and
 * plugins, so `import("ava").SharedConfiguration` and `import("@ava/typescript").Config` both
 * resolve to `any`. This file is the project's own description of the options and is kept in
 * step with the installed ava version.
 */
export interface AvaConfig {
  /** Glob patterns of the test files, relative to the project directory. */
  files?: Array<string>;
  /** File extensions ava treats as tests; `@ava/typescript` contributes its own. An array only. */
  extensions?: Array<string>;
  /** Only run tests whose titles match these patterns. */
  match?: Array<string>;
  /** Cache compiled files under `node_modules/.cache/ava`. Default `true`. */
  cache?: boolean;
  /** Test files run at once. Default: the number of CPU cores. */
  concurrency?: number;
  /** Run test files in worker threads instead of child processes. Default `true`. */
  workerThreads?: boolean;
  /** Run tests serially. Default `false`. */
  serial?: boolean;
  /** Stop after the first test failure. Default `false`. */
  failFast?: boolean;
  /** Fail tests that make no assertions. Default `true`. */
  failWithoutAssertions?: boolean;
  /** TAP output. Default `false`. */
  tap?: boolean;
  /** Verbose reporter. Default `true` in ava 8. */
  verbose?: boolean;
  /** Where snapshot files live. */
  snapshotDir?: string;
  /** Rewrite snapshots. Default `false`. */
  updateSnapshots?: boolean;
  /**
   * The inactivity timeout: milliseconds, or an `ms` string such as `10s`, `2m` or `60m`. ava
   * resets it whenever a test finishes and suspends it while a test's own `t.timeout()` runs.
   * ava's default is `10s`.
   */
  timeout?: number | string;
  /** Variables set in every worker's environment. */
  environmentVariables?: Record<string, string>;
  /** Arguments passed to the Node process (or worker) running the tests. */
  nodeArguments?: Array<string>;
  /** Modules loaded before the test files. */
  require?: Array<string>;
  /** Let `@ava/typescript` and other providers build in parallel. Default `true`. */
  utilizeParallelBuilds?: boolean;
  /** Order in which test files run. */
  sortTestFiles?: (a: string, b: string) => number;
  /** Which `nodeArguments` reach worker threads. */
  filterNodeArgumentsForWorkerThreads?: (args: Array<string>) => Array<string>;
  watchMode?: {
    /** Paths whose changes do not trigger a rerun. */
    ignoreChanges?: Array<string>;
  };
  /** `@ava/typescript`: compile with `tsc` first or not at all, and where compiled output lives. */
  typescript?: {
    compile: false | "tsc";
    /** Source directory to output directory, both ending in `/`. */
    rewritePaths: Record<string, string>;
    extensions?: Array<string>;
  };
  nonSemVerExperiments?: Record<string, boolean>;
}

/** A config file may export a factory instead of an object. */
export type AvaConfigFactory = (options: { projectDir: string }) => AvaConfig;

export type AvaConfiguration = AvaConfig | AvaConfigFactory;
