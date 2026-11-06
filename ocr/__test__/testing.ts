import { Fs } from "@d0paminedriven/fs";

const dirPath = `ocr/__test__/fixtures/private`;
const fs = new Fs(process.cwd());

const arrToArrOfArrs = <const T = unknown>(arr: readonly T[], int = 10, agg = Array.of<T[]>()) => {
  for (let i = 0; i < arr.length; i += int) {
    agg.push(arr.slice(i, i + int));
  }
  return agg;
};

if (process.argv[3] === "inspect") {
  function byFileType(target: "xlsx" | "pdf") {
    return fs
      .readDir(dirPath, { recursive: true })
      .filter((t) => t.lastIndexOf(".") !== -1 && t.slice(t.lastIndexOf(".") + 1) === target)
      .map((v) => `${dirPath}/${v}`);
  }

  const pdfs = byFileType("pdf");
  const toSubArrays = arrToArrOfArrs(pdfs, 100, Array.of<string[]>());
  console.log(pdfs.length);

  console.log(toSubArrays.length);
}

const tiffs = ["ocr/__test__/fixtures/private/001/001477.pdf", "ocr/__test__/fixtures/private/001/001478.pdf"];

const rtf = "ocr/__test__/fixtures/private/001/001703.pdf";

fs.docs
  .extractDocViaPath(rtf, "text/rtf")
  .then((t) => {
    const payload = { path: rtf, ...t };
    console.log(payload);
  })
  .catch((err) => console.error(err));

  
for (const tiff of tiffs) {
  fs.img
    .extractImgSpecsViaPath(tiff, 48 * 4096)
    .then((t) => {
      const payload = { path: tiff, ...t };
      console.log(payload);
    })
    .catch((err) => console.error(err));
}
