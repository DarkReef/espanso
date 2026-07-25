# rEspanso medical calculator windows

rEspanso should render calculator and questionnaire windows without embedding clinical formulas into the text-expander core.

## Architecture

```text
trigger / selected missed trigger
        ↓
Espanso form extension
        ↓
external calculator module
        ↓
stdout text result
        ↓
@dialog result window
```

The core is responsible only for:

- opening the form;
- collecting field values;
- passing values to an external process;
- displaying the returned result;
- preserving the ordinary Espanso trigger workflow.

The external module is responsible for:

- input validation;
- the formula or decision table;
- score interpretation;
- version and source metadata;
- clinical disclaimers;
- tests for boundary values.

## Environment contract

A form variable named `calculator_inputs` exposes values to scripts as:

```text
ESPANSO_CALCULATOR_INPUTS_<FIELD_NAME>
```

For example:

```text
ESPANSO_CALCULATOR_INPUTS_AGE=64
ESPANSO_CALCULATOR_INPUTS_SEX=Мужской
ESPANSO_CALCULATOR_INPUTS_SMOKING=Да
```

The external module prints the complete user-facing result to stdout. A non-zero exit code or stderr message is treated as a calculation failure.

## Folder layout

```text
config/
├── match/
│   └── medical-calculators.yml
└── medical-calculators/
    ├── run-calculator.ps1
    └── modules/
        ├── findrisc.ps1
        ├── has-bled.ps1
        ├── score2.ps1
        ├── cha2ds2-vasc.ps1
        └── h2fpef.ps1
```

The first repository example contains only `ui-demo.ps1`. It deliberately does not implement a clinical scale.

## Example match

```yaml
matches:
  - trigger: ":calc_demo"
    replace: |
      @dialog: Демонстрационный калькулятор
      {{calculator_result}}

    vars:
      - name: calculator_inputs
        type: form
        params:
          layout: |
            Возраст: [[age]]
            Фактор: [[factor]]
          fields:
            age:
              type: text
            factor:
              type: choice
              values: ["Нет", "Да"]

      - name: calculator_result
        type: script
        depends_on: [calculator_inputs]
        params:
          args:
            - powershell.exe
            - -NoProfile
            - -ExecutionPolicy
            - Bypass
            - -File
            - "%CONFIG%/medical-calculators/run-calculator.ps1"
            - "ui-demo"
```

## Planned clinical modules

Each scale should be delivered as an independently versioned module:

- FINDRISC;
- HAS-BLED;
- SCORE2 / SCORE2-OP;
- CHA2DS2-VASc;
- H2FPEF.

SCORE2 should not be reduced to a simplistic point sum: its module may call an HTTP service or a compiled local library while keeping the same form-to-module contract.

Before a clinical module is distributed, it should include:

- a cited authoritative source and version date;
- unit tests with published examples or validated reference cases;
- explicit input units;
- age and population applicability limits;
- a statement that the result supports, but does not replace, clinical judgment;
- no patient identifiers in debug logs.
