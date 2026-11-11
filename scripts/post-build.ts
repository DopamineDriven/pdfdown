import { Fs } from "@d0paminedriven/fs";

export type TargetUnion = "base" | "superset" | "wasm32-wasi";

export class PostBuildService extends Fs {
  constructor() {
    super(process.cwd());
  }

  private get wasiPath() {
    return "pdfdown.wasi.d.cts" as const;
  }

  private get basePath() {
    return "index.d.ts" as const;
  }

  private get supersetPath() {
    return "ocr/index.d.ts" as const;
  }

  private tPath(target: TargetUnion) {
    const path =
      target === "base"
        ? this.basePath
        : target === "superset"
          ? this.supersetPath
          : target === "wasm32-wasi"
            ? this.wasiPath
            : undefined;
    if (!path) {
      throw new Error(`incorrect target ${target} provided to post-build.ts`);
    } else {
      return path;
    }
  }

  public exe(target: TargetUnion) {
    let t: string;
    const path = this.tPath(target);
    let s: string | undefined;
    if (this.exists(path)) {
      s = this.fileToBuffer(path).toString("utf-8");
    } else {
      s = undefined;
    }
    if (!s) return;
    const parsed = this.parseComments(s);
    const filter = parsed
      ? typeof parsed === "string"
        ? undefined
        : parsed?.filter((t) => t.includes("eslint-disable"))
      : undefined;
    if (filter && filter.length > 0) {
      t = s.replaceAll("/* eslint-disable */", "");
    } else {
      t = s;
    }
    try {
      this.withWs(path, t);
    } catch (err) {
      console.warn(err);
    } finally {
      const content = this.fileToBuffer("index.d.ts").toString("utf-8");
      if (path === "pdfdown.wasi.d.cts" && content.includes("eslint-disable")) {
        const parsed = this.parseComments(content);
        const replaceEslintDisable = parsed
          ? typeof parsed === "string"
            ? undefined
            : parsed?.filter((t) => t.includes("eslint-disable"))
          : undefined;
        if (replaceEslintDisable && replaceEslintDisable?.length > 0) {
          this.withWs("index.d.ts", content.replaceAll("/* eslint-disable */", ""));
        }
      }
    }
  }
  /**
   *
   * pulled from a package I wrote circa 2022 through 2024 -- it works -- it was used to find xr-ignore flags there to prevent editing generated files. tomato, tomat-oh
   */
  private parseComments<const T extends string>(props: T) {
    // eslint-disable-next-line
    return /(?:(?:\/\*(?:[^*]|(?:\*+[^*\/]))*\*+\/)|(?:(?<!\:|\\\|\')\/\/.*))/gm.test(props) === true
      ? props.match(
          // eslint-disable-next-line
          /(?:(?:\/\*(?:[^*]|(?:\*+[^*\/]))*\*+\/)|(?:(?<!\:|\\\|\')\/\/.*))/gm
        )
      : props;
  }
}

if (
  process.argv[3] === "modify" &&
  (process.argv[5] === "base" || process.argv[5] === "superset" || process.argv[5] === "wasm32-wasi")
) {
  const service = new PostBuildService();
  service.exe(process.argv[5]);
}
