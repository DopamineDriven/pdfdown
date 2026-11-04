// Bound Tesseract's OpenMP pool before libtesseract loads: libgomp reads the environment in its
// constructor, so this cannot be done from Rust once the native module is loaded. One OpenMP
// thread per OCR worker; parallelism comes from OcrOptions.maxThreads (rayon). Measured on an
// 8-core box: 7.8 s/page with the default pool, 0.6 s/page with the limit. Users who set either
// OpenMP variable themselves keep their setting.
if (process.env.OMP_THREAD_LIMIT === undefined && process.env.OMP_NUM_THREADS === undefined) {
  process.env.OMP_THREAD_LIMIT = "1";
}

module.exports = require("./pdfdown_ocr.napi.js");
