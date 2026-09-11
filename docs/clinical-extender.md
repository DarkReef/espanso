# Clinical Extender

Clinical Extender is a local deterministic composer for reusable clinical text fragments. It is exposed as a first-class top-level section of rEspanso Match Studio alongside Rules, Settings, Rhai and AI/MCP.

## Privacy model

The current visit text is kept in memory only. Saving Clinical Extender writes only the reusable template library to:

`<config_root>/clinical_extender/nosologies.yml`

The template library must not contain patient identifiers or patient-specific data.

## Composition model

A visit is assembled in this order:

1. BASE therapeutic profile.
2. Matching wildcard templates such as `I11.*`.
3. Exact diagnosis templates such as `I11.9`.
4. Aliases such as `АГ`, `ГБ`, `ХСН` resolve to their template.
5. Manual clinician edits remain in the visit editor until an explicit full rebuild.

Structured investigations use stable IDs and are de-duplicated before rendering. Free-text sections are appended in deterministic order while exact duplicate fragments are removed.

## Main workflow

Open the top-level `Clinical Extender` section and use:

- `Осмотр` to enter codes/aliases and compose the current note.
- `Редактор нозологий` to create or edit reusable templates.
- `Обновить из шаблонов` to refresh only untouched fields.
- `Пересобрать всё` to intentionally replace manual edits with the current templates.
- Per-section copy buttons or `Копировать весь осмотр` to move text into any HIS/EMR.

## Starter library

A new library starts with a compact general-practice seed set. It is deliberately editable and is intended as a scaffold, not as an immutable clinical protocol. The current seed covers common cardiovascular, metabolic, renal, respiratory, gastrointestinal, thyroid, hematology and musculoskeletal patterns.

Before clinical use, adapt the texts and investigation set to local clinical guidelines, local orders, available diagnostics and your organisation's documentation requirements.

## Template format

Each template contains:

- `is_base`: whether it applies to every composed visit.
- `code_pattern`: exact ICD-10 code or a trailing-star prefix such as `I48.*`.
- `title`: human-readable name.
- `aliases`: shorthand accepted in the diagnosis input.
- `sections`: complaints, disease history, life history, past diseases, objective status, free examination-plan text, treatment and recommendations.
- `investigation_ids`: references to the structured investigation catalog.

Example:

```yaml
- is_base: false
  code_pattern: I11.9
  title: Гипертензивная болезнь сердца без сердечной недостаточности
  aliases:
    - АГ
    - ГБ
  sections:
    disease_history: >-
      Артериальной гипертензией страдает длительное время.
  investigation_ids:
    - echocardiography
    - urine_acr
```

The UI is the preferred editor because it keeps investigation IDs consistent.

## Safety boundary

Clinical Extender is deterministic template composition, not autonomous diagnosis or treatment. Its starter content is an editable draft. The clinician remains responsible for checking that generated text matches the actual patient and current evidence/guidelines. AI/MCP stays in a separate top-level section and should only rewrite text after local de-identification and explicit review.
