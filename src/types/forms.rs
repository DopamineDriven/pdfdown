use napi_derive::napi;

// ── AcroForm field types (JS boundary) ──────────────────────────
//
// Read-only per-page form field extraction. `PageFormField` is a structured enum: napi-derive
// emits it as a TypeScript discriminated union keyed by `type`, one variant per field kind, every
// variant carrying the common fields plus its kind-specific ones. Buttons report the appearance
// state name (`on_state`) and the /Opt export value (`export_value`) separately. Presentation
// data (/DA, /Q, /MK, appearance streams) is deliberately not surfaced.

#[napi(object)]
#[derive(Clone, Debug, PartialEq)]
pub struct ChoiceOption {
  pub export_value: String,
  pub display_value: String,
}

/// A logical field with no widget placed on any page. Keeps the full field payload; only
/// geometry and per-widget state are absent.
#[napi(object)]
#[derive(Clone, Debug, PartialEq)]
pub struct UnplacedFormField {
  /// `"N G obj"`, or `"<parent fieldObjectId>/kid I"` / `"/Fields I"` when the field
  /// dictionary itself is written inline.
  pub field_object_id: String,
  pub name: String,
  pub partial_name: String,
  #[napi(js_name = "type", ts_type = "FormFieldType")]
  pub kind: String,
  pub value: Option<String>,
  /// Multi-select list boxes.
  pub values: Option<Vec<String>>,
  pub default_value: Option<String>,
  pub tooltip: Option<String>,
  pub mapping_name: Option<String>,
  /// Choice fields.
  pub options: Option<Vec<ChoiceOption>>,
  pub max_len: Option<u32>,
  pub read_only: bool,
  pub required: bool,
  pub no_export: bool,
  pub has_actions: bool,
  pub multiline: bool,
  pub password: bool,
  pub comb: bool,
  pub rich_text: bool,
  pub editable: bool,
  pub multi_select: bool,
  /// Signature fields: signature data present in /V (not cryptographic validation).
  pub signed: bool,
}

/// Document-level AcroForm facts. Present on `PdfMeta` / `PdfDocument` when `/AcroForm` exists
/// or orphan widgets were found.
#[napi(object)]
#[derive(Clone, Debug, PartialEq)]
pub struct FormInfo {
  /// `/AcroForm` present (false when only orphan widgets were found).
  pub has_acro_form: bool,
  /// `/AcroForm /XFA` present (XFA is not parsed).
  pub has_xfa: bool,
  pub need_appearances: bool,
  pub sig_flags: u32,
  /// Logical fields, placed + unplaced; may differ from `totalFormFields`.
  pub field_count: u32,
  /// Placements; always equals `totalFormFields` and `formFields.length`.
  pub widget_count: u32,
  pub unplaced_fields: Vec<UnplacedFormField>,
  /// The `/Kids` depth cap was hit somewhere: the result is partial.
  pub truncated: bool,
  /// Dangling references, non-array `/Annots`, non-dictionary `/Kids` entries, depth cap.
  pub warnings: Vec<String>,
}

/// The fields every `PageFormField` variant carries.
pub(crate) struct FieldCommon {
  pub page: u32,
  pub rect: Vec<f64>,
  pub object_id: String,
  pub field_object_id: String,
  pub name: String,
  pub partial_name: String,
  pub widget_index: u32,
  pub widget_count: u32,
  pub value: Option<String>,
  pub default_value: Option<String>,
  pub tooltip: Option<String>,
  pub mapping_name: Option<String>,
  pub read_only: bool,
  pub required: bool,
  pub no_export: bool,
  pub has_actions: bool,
}

/// Declares the common fields once and stamps them into every variant of the napi structured
/// enum, the internal kind enum, the constructor and the common-field accessors.
macro_rules! page_form_field {
  // Kind-specific field types are written in brackets and forwarded as raw token trees: a `$ty:ty`
  // fragment reaches napi-derive wrapped in an invisible group, its `Option<…>` check misses, and a
  // `None` would be emitted as `null` instead of being omitted like every other optional field.
  ($($variant:ident { $($field:ident : [$($ty:tt)+]),* $(,)? }),+ $(,)?) => {
    #[napi(object_from_js = false)]
    pub enum PageFormField {
      $($variant {
        page: u32,
        rect: Vec<f64>,
        object_id: String,
        field_object_id: String,
        name: String,
        partial_name: String,
        widget_index: u32,
        widget_count: u32,
        value: Option<String>,
        default_value: Option<String>,
        tooltip: Option<String>,
        mapping_name: Option<String>,
        read_only: bool,
        required: bool,
        no_export: bool,
        has_actions: bool,
        $($field: $($ty)+,)*
      },)+
    }

    /// Kind-specific payload, built by the extractor and combined with `FieldCommon`.
    pub(crate) enum FieldKind {
      $($variant { $($field: $($ty)+,)* },)+
    }

    impl FieldKind {
      /// The variant name, identical to the `type` literal of the matching `PageFormField`.
      pub(crate) fn kind_name(&self) -> &'static str {
        match self { $(FieldKind::$variant { .. } => stringify!($variant),)+ }
      }
    }

    impl PageFormField {
      pub(crate) fn new(c: FieldCommon, kind: FieldKind) -> Self {
        match kind {
          $(FieldKind::$variant { $($field,)* } => PageFormField::$variant {
            page: c.page,
            rect: c.rect,
            object_id: c.object_id,
            field_object_id: c.field_object_id,
            name: c.name,
            partial_name: c.partial_name,
            widget_index: c.widget_index,
            widget_count: c.widget_count,
            value: c.value,
            default_value: c.default_value,
            tooltip: c.tooltip,
            mapping_name: c.mapping_name,
            read_only: c.read_only,
            required: c.required,
            no_export: c.no_export,
            has_actions: c.has_actions,
            $($field,)*
          },)+
        }
      }

      /// Variant name; used by tests and by the invoice phases, not by extraction itself.
      #[allow(dead_code)]
      pub(crate) fn kind_name(&self) -> &'static str {
        match self { $(PageFormField::$variant { .. } => stringify!($variant),)+ }
      }

      pub(crate) fn page(&self) -> u32 {
        match self { $(PageFormField::$variant { page, .. } => *page,)+ }
      }

      pub(crate) fn rect(&self) -> &[f64] {
        match self { $(PageFormField::$variant { rect, .. } => rect,)+ }
      }

      pub(crate) fn name(&self) -> &str {
        match self { $(PageFormField::$variant { name, .. } => name,)+ }
      }

      pub(crate) fn object_id(&self) -> &str {
        match self { $(PageFormField::$variant { object_id, .. } => object_id,)+ }
      }
    }
  };
}

page_form_field! {
  Text { multiline: [bool], password: [bool], comb: [bool], rich_text: [bool], max_len: [Option<u32>] },
  Checkbox { on_state: [Option<String>], export_value: [Option<String>], checked: [bool] },
  Radio { on_state: [Option<String>], export_value: [Option<String>], selected: [bool] },
  ComboBox { options: [Vec<ChoiceOption>], editable: [bool] },
  ListBox { options: [Vec<ChoiceOption>], multi_select: [bool], values: [Option<Vec<String>>] },
  PushButton {},
  Signature { signed: [bool] },
  Unknown {}
}

#[cfg(test)]
mod tests;
