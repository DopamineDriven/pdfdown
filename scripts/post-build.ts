import { Fs } from "@d0paminedriven/fs";
export class PostBuildService extends Fs {
  constructor() {
    super(process.cwd());
  }

  private toPrepend(content: string) {
    return `import type {
  AtLeastOne,
  Repeater,
  Rm,
  Include,
  Without,
  XOR,
  CTR,
  RTC,
  IsExact,
  Equal,
  UTR,
  OnlyOptional,
  OnlyRequired,
  IsOptional,
  Unenumerate
} from "@d0paminedriven/type-utils";
export type {
  AtLeastOne,
  Repeater,
  Rm,
  Include,
  Without,
  XOR,
  CTR,
  RTC,
  IsExact,
  Equal,
  UTR,
  OnlyOptional,
  OnlyRequired,
  IsOptional,
  Unenumerate
};
// ── Derived aliases over the generated declarations below (TypeScript hoists type aliases) ──

export type FormFieldType = PageFormField["type"];
export type FormFieldByType = UTR<PageFormField, "type", true>;

/**
 * Payload of one or more variants, selected by discriminant. Pass a single key
 * for that variant's fields, or a union of keys for the union of their payloads.
 *
 * @example
 * type Sig = FormFieldPayload<"Signature">;              // { signed: boolean; page: number; ... }
 * type Buttons = FormFieldPayload<"Checkbox" | "Radio">; // either payload
 */
export type FormFieldPayload<T extends keyof FormFieldByType = keyof FormFieldByType> = {
  [P in T]: FormFieldByType[P];
}[T];

${content}
`;
  }

  private reader(target: "base" | "superset") {
    const path = target === "base" ? "index.d.ts" : target === "superset" ? "ocr/index.d.ts" : undefined;
    if (path && this.exists(path)) {
      return this.toPrepend(this.fileToBuffer(path).toString("utf-8"));
    } else console.warn("index.d.ts does not exist for types to be prepended");
  }
  public exe(target: "base" | "superset") {
    let t: string;
    const template = this.reader(target);

    if (!template) return;
    const parsed = this.parseComments(template);
    const filter = parsed
      ? typeof parsed === "string"
        ? undefined
        : parsed?.filter((t) => t.includes("eslint-disable"))
      : undefined;
    if (filter && filter.length > 0) {
      t = template.replaceAll("/* eslint-disable */", "");
    } else {
      t = template;
    }
    const path = target === "base" ? "index.d.ts" : target === "superset" ? "ocr/index.d.ts" : undefined;
    if (!path) return;
    try {
      this.withWs(path, t);
    } catch (err) {
      console.warn(err);
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

if (process.argv[3] === "modify" && (process.argv[5] === "base" || process.argv[5] === "superset")) {
  const service = new PostBuildService();
  service.exe(process.argv[5]);
}
