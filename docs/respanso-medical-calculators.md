# rEspanso medical calculator windows

rEspanso renders calculator and questionnaire windows without embedding clinical formulas into the text-expander core. The editable calculation logic is interpreted by the Rhai runtime bundled inside `rEspanso.exe`.

## Architecture

```text
trigger / selected missed trigger
        ↓
Espanso form extension
        ↓
embedded Rhai module (.rhai)
        ↓
text or structured result
        ↓
@dialog result window
```

The core is responsible only for:

- opening the form;
- collecting field values;
- passing values to the embedded interpreter;
- enforcing execution limits and allowed script locations;
- displaying the returned result;
- preserving the ordinary Espanso trigger workflow.

The editable Rhai module is responsible for:

- input validation;
- the formula or decision table;
- score interpretation;
- version and source metadata;
- clinical disclaimers;
- tests for boundary values.

Rhai does not require Rust, Cargo, Visual Studio, Node.js, Python or PowerShell on the workstation. The interpreter is linked into `rEspanso.exe`; changing a `.rhai` file takes effect after the normal configuration reload and does not require recompilation.

## Input contract

A form variable named `calculator_inputs` is passed to the Rhai function as a map:

```rhai
fn calculate(input) {
    let age = parse_int(input.age);
    let smoking = input.smoking == "Да";

    // Formula belongs here, outside the rEspanso core.
    `Возраст: ${age}; курение: ${smoking}`
}
```

Form values currently arrive as strings. Numeric modules should validate and parse them explicitly with `parse_int` or `parse_float`.

A Rhai function may return:

- a string — exposed as a normal single variable;
- a map — exposed as subvariables such as `{{result.score}}` and `{{result.text}}`;
- a number or boolean — converted to text.

## Folder layout

```text
config/
├── match/
│   └── medical-calculators.yml
└── medical-calculators/
    └── modules/
        ├── findrisc.rhai
        ├── has-bled.rhai
        ├── score2.rhai
        ├── cha2ds2-vasc.rhai
        └── h2fpef.rhai
```

Scripts are allowed only below the active config or packages directory. The embedded engine has no host-registered filesystem, network or process API. Dynamic `eval` and `import` are disabled, and execution is limited by operation count, call depth, expression depth, string size, array size and map size.

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
        type: rhai
        depends_on: [calculator_inputs]
        params:
          path: "%CONFIG%/medical-calculators/modules/ui-demo.rhai"
          function: "calculate"
          input: "calculator_inputs"
```

Example module:

```rhai
fn calculate(input) {
    let age = parse_int(input.age);
    let factor_points = if input.factor == "Да" { 1 } else { 0 };
    let demo_score = age / 10 + factor_points;

    `Демонстрационный результат: ${demo_score}`
}
```

This repository example deliberately does not implement a validated clinical scale.

## Planned clinical modules

Each scale should be delivered as an independently versioned module:

- FINDRISC;
- HAS-BLED;
- SCORE2 / SCORE2-OP;
- CHA2DS2-VASc;
- H2FPEF.

SCORE2 should not be reduced to an improvised point sum. Its module needs validated coefficients/tables for the intended population, explicit applicability limits and reference cases.

Before a clinical module is distributed, it should include:

- a cited authoritative source and version date;
- unit tests with published examples or independently validated reference cases;
- explicit input units;
- age and population applicability limits;
- a statement that the result supports, but does not replace, clinical judgment;
- no patient identifiers in debug logs;
- a module version and checksum.
