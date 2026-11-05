//! Workbooks: an `.xlsx` is a zip of XML parts. `package` opens the zip within limits and resolves
//! parts through relationships; `workbook` reads the parts into a sparse cell model; `format`
//! renders display text from format codes; `project` places cells on a logical page.

pub(crate) mod format;
pub(crate) mod package;
pub(crate) mod workbook;
