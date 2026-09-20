use eframe::egui;
use rhai::{Dynamic, Engine, Scope};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

pub const TEMPLATE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TemplatePackage {
    pub schema_version: u32,
    pub package: PackageMetadata,
    #[serde(default)]
    pub fields: Vec<TemplateField>,
    #[serde(default)]
    pub calculations: Vec<Calculation>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub sections: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default = "default_package_status")]
    pub status: String,
    #[serde(default)]
    pub description: String,
}

fn default_package_status() -> String {
    "draft".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TemplateField {
    pub id: String,
    pub label: String,
    pub kind: FieldKind,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<Value>,
    #[serde(default)]
    pub choices: Vec<String>,
    #[serde(default)]
    pub source: Option<SourceReference>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Text,
    Date,
    Integer,
    Decimal,
    Choice,
    Boolean,
    Computed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceReference {
    pub document: String,
    #[serde(default)]
    pub section: Option<String>,
    #[serde(default)]
    pub effective_date: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Calculation {
    pub id: String,
    pub output: String,
    #[serde(flatten)]
    pub kind: CalculationKind,
    #[serde(default)]
    pub inputs: BTreeMap<String, String>,
    #[serde(default)]
    pub precision: Option<u32>,
    #[serde(default)]
    pub source: Option<SourceReference>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CalculationKind {
    Builtin { builtin: BuiltinCalculator },
    Formula { expression: String },
    Script { script: String },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BuiltinCalculator {
    Bmi,
    #[serde(rename = "ckd_epi_2021")]
    CkdEpi2021,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Rule {
    pub id: String,
    pub when: Condition,
    #[serde(default)]
    pub actions: Vec<Action>,
    #[serde(default)]
    pub source: Option<SourceReference>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Condition {
    All { conditions: Vec<Condition> },
    Any { conditions: Vec<Condition> },
    Not { condition: Box<Condition> },
    Exists { var: String },
    Eq { left: Operand, right: Operand },
    Ne { left: Operand, right: Operand },
    Lt { left: Operand, right: Operand },
    Lte { left: Operand, right: Operand },
    Gt { left: Operand, right: Operand },
    Gte { left: Operand, right: Operand },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Operand {
    Variable { var: String },
    Literal { value: Value },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Set { field: String, value: Operand },
    Require { field: String },
    Show { field: String },
    Hide { field: String },
    Warning {
        message: String,
        #[serde(default)]
        level: Option<String>,
    },
    Recommendation { message: String },
    AppendText { section: String, text: String },
    ActivateModule { id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineMessage {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvaluationResult {
    pub values: BTreeMap<String, Value>,
    pub sections: BTreeMap<String, String>,
    pub required_fields: Vec<String>,
    pub visible_fields: Vec<String>,
    pub hidden_fields: Vec<String>,
    pub recommendations: Vec<String>,
    pub warnings: Vec<EngineMessage>,
    pub activated_rules: Vec<String>,
    pub activated_modules: Vec<String>,
    pub errors: Vec<EngineMessage>,
}

impl EvaluationResult {
    fn new(values: BTreeMap<String, Value>) -> Self {
        Self {
            values,
            sections: BTreeMap::new(),
            required_fields: Vec::new(),
            visible_fields: Vec::new(),
            hidden_fields: Vec::new(),
            recommendations: Vec::new(),
            warnings: Vec::new(),
            activated_rules: Vec::new(),
            activated_modules: Vec::new(),
            errors: Vec::new(),
        }
    }
}

impl TemplatePackage {
    pub fn from_json(source: &str) -> Result<Self, String> {
        let package: Self =
            serde_json::from_str(source).map_err(|error| format!("Ошибка JSON: {error}"))?;
        package.validate()?;
        Ok(package)
    }

    pub fn to_pretty_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|error| error.to_string())
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != TEMPLATE_SCHEMA_VERSION {
            return Err(format!(
                "Неподдерживаемая версия схемы {}. Поддерживается {}",
                self.schema_version, TEMPLATE_SCHEMA_VERSION
            ));
        }
        for (name, value) in [
            ("package.id", self.package.id.trim()),
            ("package.name", self.package.name.trim()),
            ("package.version", self.package.version.trim()),
        ] {
            if value.is_empty() {
                return Err(format!("Поле {name} не заполнено"));
            }
        }

        let mut field_ids = BTreeSet::new();
        for field in &self.fields {
            if field.id.trim().is_empty() {
                return Err("Найдено поле без id".to_owned());
            }
            if !field_ids.insert(field.id.as_str()) {
                return Err(format!("Повторяющийся id поля: {}", field.id));
            }
            if field.kind == FieldKind::Choice && field.choices.is_empty() {
                return Err(format!(
                    "Поле выбора {} не содержит вариантов choices",
                    field.id
                ));
            }
        }

        let mut calculation_ids = BTreeSet::new();
        let mut outputs = BTreeSet::new();
        for calculation in &self.calculations {
            if !calculation_ids.insert(calculation.id.as_str()) {
                return Err(format!(
                    "Повторяющийся id вычисления: {}",
                    calculation.id
                ));
            }
            if !field_ids.contains(calculation.output.as_str()) {
                return Err(format!(
                    "Вычисление {} пишет в неизвестное поле {}",
                    calculation.id, calculation.output
                ));
            }
            if !outputs.insert(calculation.output.as_str()) {
                return Err(format!(
                    "Несколько вычислений пишут в одно поле {}",
                    calculation.output
                ));
            }
            for field in calculation.inputs.values() {
                if !field_ids.contains(field.as_str()) {
                    return Err(format!(
                        "Вычисление {} зависит от неизвестного поля {}",
                        calculation.id, field
                    ));
                }
            }
            match &calculation.kind {
                CalculationKind::Formula { expression } if expression.trim().is_empty() => {
                    return Err(format!("Пустая формула в {}", calculation.id));
                }
                CalculationKind::Script { script } if script.trim().is_empty() => {
                    return Err(format!("Пустой Rhai-скрипт в {}", calculation.id));
                }
                CalculationKind::Script { script } if script.len() > 32_768 => {
                    return Err(format!(
                        "Rhai-скрипт {} превышает лимит 32 KiB",
                        calculation.id
                    ));
                }
                _ => {}
            }
        }

        let mut rule_ids = BTreeSet::new();
        for rule in &self.rules {
            if !rule_ids.insert(rule.id.as_str()) {
                return Err(format!("Повторяющийся id правила: {}", rule.id));
            }
            let mut refs = BTreeSet::new();
            collect_condition_variables(&rule.when, &mut refs);
            for field in refs {
                if !field_ids.contains(field.as_str()) {
                    return Err(format!(
                        "Правило {} ссылается на неизвестное поле {}",
                        rule.id, field
                    ));
                }
            }
            for action in &rule.actions {
                match action {
                    Action::Set { field, value } => {
                        if !field_ids.contains(field.as_str()) {
                            return Err(format!(
                                "Правило {} пишет в неизвестное поле {}",
                                rule.id, field
                            ));
                        }
                        if let Operand::Variable { var } = value {
                            if !field_ids.contains(var.as_str()) {
                                return Err(format!(
                                    "Правило {} читает неизвестное поле {}",
                                    rule.id, var
                                ));
                            }
                        }
                    }
                    Action::Require { field }
                    | Action::Show { field }
                    | Action::Hide { field } => {
                        if !field_ids.contains(field.as_str()) {
                            return Err(format!(
                                "Правило {} использует неизвестное поле {}",
                                rule.id, field
                            ));
                        }
                    }
                    Action::AppendText { section, .. } => {
                        if !self.sections.contains_key(section) {
                            return Err(format!(
                                "Правило {} добавляет текст в неизвестную секцию {}",
                                rule.id, section
                            ));
                        }
                    }
                    Action::Warning { .. }
                    | Action::Recommendation { .. }
                    | Action::ActivateModule { .. } => {}
                }
            }
        }

        Ok(())
    }

    pub fn evaluate(&self, input: &Value) -> EvaluationResult {
        let mut values = BTreeMap::new();
        flatten_value("", input, &mut values);

        for field in &self.fields {
            if !values.contains_key(&field.id) {
                if let Some(value) = &field.default {
                    values.insert(field.id.clone(), value.clone());
                }
            }
        }

        let mut result = EvaluationResult::new(values);
        let mut required = self
            .fields
            .iter()
            .filter(|field| field.required)
            .map(|field| field.id.clone())
            .collect::<BTreeSet<_>>();
        let mut visible = self
            .fields
            .iter()
            .map(|field| field.id.clone())
            .collect::<BTreeSet<_>>();
        let mut hidden = BTreeSet::new();
        let mut recommendations = BTreeSet::new();
        let mut warnings = BTreeMap::<String, EngineMessage>::new();
        let mut activated_rules = BTreeSet::new();
        let mut activated_modules = BTreeSet::new();
        let mut section_appends = BTreeMap::<String, Vec<String>>::new();

        // Fixed-point evaluation lets a rule set a value that unlocks a later
        // calculation or another rule without relying on JSON ordering.
        for _ in 0..16 {
            let before = result.values.clone();
            self.evaluate_calculations(&mut result);

            for rule in &self.rules {
                match evaluate_condition(&rule.when, &result.values) {
                    Ok(true) => {
                        activated_rules.insert(rule.id.clone());
                        for action in &rule.actions {
                            apply_action(
                                action,
                                &mut result.values,
                                &mut required,
                                &mut visible,
                                &mut hidden,
                                &mut recommendations,
                                &mut warnings,
                                &mut activated_modules,
                                &mut section_appends,
                            );
                        }
                    }
                    Ok(false) => {}
                    Err(error) => push_unique_message(
                        &mut result.errors,
                        EngineMessage {
                            code: format!("rule:{}", rule.id),
                            message: error,
                        },
                    ),
                }
            }

            if result.values == before {
                break;
            }
        }

        for field in &required {
            let missing = result
                .values
                .get(field)
                .is_none_or(|value| value.is_null() || value.as_str().is_some_and(str::is_empty));
            if missing {
                push_unique_message(
                    &mut result.errors,
                    EngineMessage {
                        code: format!("required:{field}"),
                        message: format!("Не заполнено обязательное поле {field}"),
                    },
                );
            }
        }

        result.required_fields = required.into_iter().collect();
        result.visible_fields = visible.into_iter().collect();
        result.hidden_fields = hidden.into_iter().collect();
        result.recommendations = recommendations.into_iter().collect();
        result.warnings = warnings.into_values().collect();
        result.activated_rules = activated_rules.into_iter().collect();
        result.activated_modules = activated_modules.into_iter().collect();

        for (section, template) in &self.sections {
            let mut rendered = render_text(template, &result.values);
            if let Some(additions) = section_appends.get(section) {
                for addition in additions {
                    if !rendered.trim().is_empty() {
                        rendered.push_str("\n");
                    }
                    rendered.push_str(&render_text(addition, &result.values));
                }
            }
            result.sections.insert(section.clone(), rendered);
        }

        result
    }

    fn evaluate_calculations(&self, result: &mut EvaluationResult) {
        let mut pending = self.calculations.iter().collect::<Vec<_>>();
        for _ in 0..=self.calculations.len() {
            if pending.is_empty() {
                return;
            }
            let mut next = Vec::new();
            let mut progressed = false;

            for calculation in pending {
                if calculation
                    .inputs
                    .values()
                    .any(|field| !result.values.contains_key(field))
                {
                    next.push(calculation);
                    continue;
                }

                match evaluate_calculation(calculation, &result.values) {
                    Ok(value) => {
                        let changed = result
                            .values
                            .get(&calculation.output)
                            .is_none_or(|current| current != &value);
                        if changed {
                            result.values.insert(calculation.output.clone(), value);
                        }
                        progressed = true;
                    }
                    Err(error) => push_unique_message(
                        &mut result.errors,
                        EngineMessage {
                            code: format!("calculation:{}", calculation.id),
                            message: error,
                        },
                    ),
                }
            }

            if !progressed {
                for calculation in next {
                    let missing = calculation
                        .inputs
                        .values()
                        .filter(|field| !result.values.contains_key(*field))
                        .cloned()
                        .collect::<Vec<_>>();
                    push_unique_message(
                        &mut result.errors,
                        EngineMessage {
                            code: format!("dependency:{}", calculation.id),
                            message: format!(
                                "Не удалось вычислить {}: отсутствуют зависимости {}",
                                calculation.output,
                                missing.join(", ")
                            ),
                        },
                    );
                }
                return;
            }
            pending = next;
        }
    }
}

fn collect_condition_variables(condition: &Condition, target: &mut BTreeSet<String>) {
    match condition {
        Condition::All { conditions } | Condition::Any { conditions } => {
            for condition in conditions {
                collect_condition_variables(condition, target);
            }
        }
        Condition::Not { condition } => collect_condition_variables(condition, target),
        Condition::Exists { var } => {
            target.insert(var.clone());
        }
        Condition::Eq { left, right }
        | Condition::Ne { left, right }
        | Condition::Lt { left, right }
        | Condition::Lte { left, right }
        | Condition::Gt { left, right }
        | Condition::Gte { left, right } => {
            for operand in [left, right] {
                if let Operand::Variable { var } = operand {
                    target.insert(var.clone());
                }
            }
        }
    }
}

fn flatten_value(prefix: &str, value: &Value, target: &mut BTreeMap<String, Value>) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten_value(&path, value, target);
            }
        }
        _ if !prefix.is_empty() => {
            target.insert(prefix.to_owned(), value.clone());
        }
        _ => {}
    }
}

fn evaluate_calculation(
    calculation: &Calculation,
    values: &BTreeMap<String, Value>,
) -> Result<Value, String> {
    let mut inputs = BTreeMap::new();
    for (alias, field) in &calculation.inputs {
        let value = values
            .get(field)
            .ok_or_else(|| format!("Нет значения поля {field}"))?;
        inputs.insert(alias.clone(), value.clone());
    }

    let value = match &calculation.kind {
        CalculationKind::Builtin { builtin } => match builtin {
            BuiltinCalculator::Bmi => calculate_bmi(&inputs)?,
            BuiltinCalculator::CkdEpi2021 => calculate_ckd_epi_2021(&inputs)?,
        },
        CalculationKind::Formula { expression } => {
            evaluate_rhai(expression, &inputs, true)?
        }
        CalculationKind::Script { script } => evaluate_rhai(script, &inputs, false)?,
    };

    Ok(round_json_number(value, calculation.precision))
}

fn input_f64(inputs: &BTreeMap<String, Value>, key: &str) -> Result<f64, String> {
    let value = inputs
        .get(key)
        .ok_or_else(|| format!("Не задан вход {key}"))?;
    if let Some(number) = value.as_f64() {
        return Ok(number);
    }
    if let Some(text) = value.as_str() {
        return text
            .replace(',', ".")
            .parse::<f64>()
            .map_err(|_| format!("Вход {key} не является числом"));
    }
    Err(format!("Вход {key} не является числом"))
}

fn input_string<'a>(inputs: &'a BTreeMap<String, Value>, key: &str) -> Result<&'a str, String> {
    inputs
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Вход {key} не является строкой"))
}

fn calculate_bmi(inputs: &BTreeMap<String, Value>) -> Result<Value, String> {
    let height_cm = input_f64(inputs, "height_cm")?;
    let weight_kg = input_f64(inputs, "weight_kg")?;
    if height_cm <= 0.0 || weight_kg <= 0.0 {
        return Err("Рост и вес должны быть больше нуля".to_owned());
    }
    let height_m = height_cm / 100.0;
    number_value(weight_kg / (height_m * height_m))
}

fn calculate_ckd_epi_2021(inputs: &BTreeMap<String, Value>) -> Result<Value, String> {
    let creatinine_umol_l = input_f64(inputs, "creatinine_umol_l")?;
    let age = input_f64(inputs, "age")?;
    if creatinine_umol_l <= 0.0 || age < 18.0 {
        return Err(
            "CKD-EPI 2021 в этом модуле требует возраст >= 18 лет и креатинин > 0".to_owned(),
        );
    }

    let sex = input_string(inputs, "sex")?.trim().to_lowercase();
    let female = matches!(
        sex.as_str(),
        "female" | "f" | "женский" | "жен" | "ж"
    );
    let male = matches!(sex.as_str(), "male" | "m" | "мужской" | "муж" | "м");
    if !female && !male {
        return Err("Для CKD-EPI 2021 sex должен быть male/female".to_owned());
    }

    let creatinine_mg_dl = creatinine_umol_l / 88.4;
    let (kappa, alpha, sex_factor) = if female {
        (0.7_f64, -0.241_f64, 1.012_f64)
    } else {
        (0.9_f64, -0.302_f64, 1.0_f64)
    };
    let ratio = creatinine_mg_dl / kappa;
    let egfr = 142.0
        * ratio.min(1.0).powf(alpha)
        * ratio.max(1.0).powf(-1.200)
        * 0.9938_f64.powf(age)
        * sex_factor;
    number_value(egfr)
}

fn evaluate_rhai(
    source: &str,
    inputs: &BTreeMap<String, Value>,
    expression_only: bool,
) -> Result<Value, String> {
    // Rhai has no filesystem/network/OS access unless host functions are
    // explicitly registered. This engine registers none and caps operations.
    let mut engine = Engine::new();
    engine.set_max_operations(50_000);
    let mut scope = Scope::new();

    for (name, value) in inputs {
        scope.push_dynamic(name.clone(), json_to_dynamic(value)?);
    }

    let result = if expression_only {
        engine
            .eval_expression_with_scope::<Dynamic>(&mut scope, source)
            .map_err(|error| format!("Ошибка формулы Rhai: {error}"))?
    } else {
        engine
            .eval_with_scope::<Dynamic>(&mut scope, source)
            .map_err(|error| format!("Ошибка Rhai-скрипта: {error}"))?
    };
    dynamic_to_json(result)
}

fn json_to_dynamic(value: &Value) -> Result<Dynamic, String> {
    match value {
        Value::Null => Ok(Dynamic::UNIT),
        Value::Bool(value) => Ok(Dynamic::from_bool(*value)),
        Value::Number(value) => {
            if let Some(integer) = value.as_i64() {
                Ok(Dynamic::from_int(integer))
            } else if let Some(number) = value.as_f64() {
                Ok(Dynamic::from_float(number))
            } else {
                Err("Число не поддерживается Rhai".to_owned())
            }
        }
        Value::String(value) => Ok(Dynamic::from(value.clone())),
        Value::Array(_) | Value::Object(_) => Err(
            "Rhai-вычисления v1 принимают только скалярные входы".to_owned(),
        ),
    }
}

fn dynamic_to_json(value: Dynamic) -> Result<Value, String> {
    if value.is_unit() {
        return Ok(Value::Null);
    }
    if value.is::<bool>() {
        return Ok(Value::Bool(value.cast::<bool>()));
    }
    if value.is::<rhai::INT>() {
        return Ok(Value::from(value.cast::<rhai::INT>()));
    }
    if value.is::<rhai::FLOAT>() {
        return number_value(value.cast::<rhai::FLOAT>());
    }
    if value.is::<String>() {
        return Ok(Value::String(value.cast::<String>()));
    }
    Err(format!(
        "Rhai вернул неподдерживаемый тип {}",
        value.type_name()
    ))
}

fn number_value(number: f64) -> Result<Value, String> {
    serde_json::Number::from_f64(number)
        .map(Value::Number)
        .ok_or_else(|| "Получено некорректное числовое значение".to_owned())
}

fn round_json_number(value: Value, precision: Option<u32>) -> Value {
    let Some(precision) = precision else {
        return value;
    };
    let Some(number) = value.as_f64() else {
        return value;
    };
    let factor = 10_f64.powi(i32::try_from(precision.min(9)).unwrap_or(9));
    number_value((number * factor).round() / factor).unwrap_or(value)
}

fn evaluate_condition(
    condition: &Condition,
    values: &BTreeMap<String, Value>,
) -> Result<bool, String> {
    match condition {
        Condition::All { conditions } => {
            for condition in conditions {
                if !evaluate_condition(condition, values)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Condition::Any { conditions } => {
            for condition in conditions {
                if evaluate_condition(condition, values)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Condition::Not { condition } => Ok(!evaluate_condition(condition, values)?),
        Condition::Exists { var } => Ok(values.get(var).is_some_and(|value| !value.is_null())),
        Condition::Eq { left, right } => compare_operands(left, right, values, CompareOp::Eq),
        Condition::Ne { left, right } => compare_operands(left, right, values, CompareOp::Ne),
        Condition::Lt { left, right } => compare_operands(left, right, values, CompareOp::Lt),
        Condition::Lte { left, right } => compare_operands(left, right, values, CompareOp::Lte),
        Condition::Gt { left, right } => compare_operands(left, right, values, CompareOp::Gt),
        Condition::Gte { left, right } => compare_operands(left, right, values, CompareOp::Gte),
    }
}

#[derive(Debug, Clone, Copy)]
enum CompareOp {
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
}

fn compare_operands(
    left: &Operand,
    right: &Operand,
    values: &BTreeMap<String, Value>,
    op: CompareOp,
) -> Result<bool, String> {
    let left = resolve_operand(left, values)?;
    let right = resolve_operand(right, values)?;

    if let (Some(left_number), Some(right_number)) = (coerce_f64(&left), coerce_f64(&right)) {
        return Ok(match op {
            CompareOp::Eq => left_number == right_number,
            CompareOp::Ne => left_number != right_number,
            CompareOp::Lt => left_number < right_number,
            CompareOp::Lte => left_number <= right_number,
            CompareOp::Gt => left_number > right_number,
            CompareOp::Gte => left_number >= right_number,
        });
    }

    match op {
        CompareOp::Eq => Ok(left == right),
        CompareOp::Ne => Ok(left != right),
        CompareOp::Lt | CompareOp::Lte | CompareOp::Gt | CompareOp::Gte => {
            let left = left
                .as_str()
                .ok_or_else(|| "Для сравнения < <= > >= нужны числа или строки".to_owned())?;
            let right = right
                .as_str()
                .ok_or_else(|| "Для сравнения < <= > >= нужны числа или строки".to_owned())?;
            Ok(match op {
                CompareOp::Lt => left < right,
                CompareOp::Lte => left <= right,
                CompareOp::Gt => left > right,
                CompareOp::Gte => left >= right,
                CompareOp::Eq | CompareOp::Ne => unreachable!(),
            })
        }
    }
}

fn resolve_operand(
    operand: &Operand,
    values: &BTreeMap<String, Value>,
) -> Result<Value, String> {
    match operand {
        Operand::Variable { var } => values
            .get(var)
            .cloned()
            .ok_or_else(|| format!("Нет значения {var}")),
        Operand::Literal { value } => Ok(value.clone()),
    }
}

fn coerce_f64(value: &Value) -> Option<f64> {
    value.as_f64().or_else(|| {
        value
            .as_str()
            .and_then(|text| text.replace(',', ".").parse::<f64>().ok())
    })
}

#[allow(clippy::too_many_arguments)]
fn apply_action(
    action: &Action,
    values: &mut BTreeMap<String, Value>,
    required: &mut BTreeSet<String>,
    visible: &mut BTreeSet<String>,
    hidden: &mut BTreeSet<String>,
    recommendations: &mut BTreeSet<String>,
    warnings: &mut BTreeMap<String, EngineMessage>,
    activated_modules: &mut BTreeSet<String>,
    section_appends: &mut BTreeMap<String, Vec<String>>,
) {
    match action {
        Action::Set { field, value } => {
            if let Ok(value) = resolve_operand(value, values) {
                values.insert(field.clone(), value);
            }
        }
        Action::Require { field } => {
            required.insert(field.clone());
        }
        Action::Show { field } => {
            hidden.remove(field);
            visible.insert(field.clone());
        }
        Action::Hide { field } => {
            visible.remove(field);
            hidden.insert(field.clone());
        }
        Action::Warning { message, level } => {
            let code = format!("warning:{}", stable_message_key(message));
            warnings.entry(code.clone()).or_insert_with(|| EngineMessage {
                code,
                message: match level {
                    Some(level) if !level.trim().is_empty() => {
                        format!("[{level}] {}", render_text(message, values))
                    }
                    _ => render_text(message, values),
                },
            });
        }
        Action::Recommendation { message } => {
            recommendations.insert(render_text(message, values));
        }
        Action::AppendText { section, text } => {
            let entry = section_appends.entry(section.clone()).or_default();
            let rendered = render_text(text, values);
            if !entry.contains(&rendered) {
                entry.push(rendered);
            }
        }
        Action::ActivateModule { id } => {
            activated_modules.insert(id.clone());
        }
    }
}

fn stable_message_key(message: &str) -> String {
    message
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(48)
        .collect::<String>()
}

fn render_text(template: &str, values: &BTreeMap<String, Value>) -> String {
    let mut rendered = template.to_owned();
    for (key, value) in values {
        let marker = format!("{{{{{key}}}}}");
        if rendered.contains(&marker) {
            rendered = rendered.replace(&marker, &display_value(value));
        }
    }
    rendered
}

fn display_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => {
            if *value {
                "да".to_owned()
            } else {
                "нет".to_owned()
            }
        }
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn push_unique_message(target: &mut Vec<EngineMessage>, message: EngineMessage) {
    if !target
        .iter()
        .any(|existing| existing.code == message.code && existing.message == message.message)
    {
        target.push(message);
    }
}

pub struct ClinicalTemplateWorkspace {
    path: String,
    template_json: String,
    original_json: String,
    input_json: String,
    result_json: String,
    status: String,
}

impl ClinicalTemplateWorkspace {
    pub fn new(config_root: &Path) -> Self {
        let path = config_root
            .join("clinical_extender")
            .join("template-engine.json");
        let path_text = path.to_string_lossy().to_string();
        let template_json = fs::read_to_string(&path).unwrap_or_else(|_| example_package_json());
        let input_json = example_input_json();
        Self {
            path: path_text,
            original_json: template_json.clone(),
            template_json,
            input_json,
            result_json: String::new(),
            status: "JSON Engine готов. Активный пакет не применяется без явной проверки и сохранения."
                .to_owned(),
        }
    }

    pub fn dirty(&self) -> bool {
        self.template_json != self.original_json
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let viewport_height = ui.available_height().max(120.0);
        egui::ScrollArea::vertical()
            .id_salt("clinical_template_engine_page")
            .max_height(viewport_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.heading("Clinical Template Engine · JSON / Rules");
                ui.label(
                    "Пакет описывает именованные поля, вычисления, зависимости, правила, секции и источники. JSON сначала валидируется; движок не исполняет файловые или сетевые функции.",
                );

                ui.horizontal_wrapped(|ui| {
                    ui.label("Файл:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.path)
                            .desired_width(520.0)
                            .hint_text("clinical_extender/template-engine.json"),
                    );
                    if ui.button("Загрузить JSON").clicked() {
                        self.load_file();
                    }
                    if ui.button("Проверить").clicked() {
                        self.validate_current();
                    }
                    if ui.button("Форматировать").clicked() {
                        self.format_current();
                    }
                    if ui.button("Сохранить проверенный").clicked() {
                        self.save_current();
                    }
                });

                if self.dirty() {
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        "JSON изменён и ещё не сохранён",
                    );
                }
                ui.label(egui::RichText::new(&self.status).weak());
                ui.separator();

                ui.label(egui::RichText::new("Template Package JSON").strong());
                ui.add(
                    egui::TextEdit::multiline(&mut self.template_json)
                        .code_editor()
                        .desired_rows(24)
                        .desired_width(f32::INFINITY),
                );

                ui.separator();
                ui.label(egui::RichText::new("Тестовый контекст пациента").strong());
                ui.label(
                    egui::RichText::new(
                        "Контекст допускается как вложенный JSON; движок преобразует его в patient.age, labs.creatinine_umol_l и т. п.",
                    )
                    .weak(),
                );
                ui.add(
                    egui::TextEdit::multiline(&mut self.input_json)
                        .code_editor()
                        .desired_rows(10)
                        .desired_width(f32::INFINITY),
                );

                ui.horizontal_wrapped(|ui| {
                    if ui.button("Вычислить").clicked() {
                        self.evaluate_current();
                    }
                    if ui.button("Копировать результат").clicked() {
                        ui.ctx().copy_text(self.result_json.clone());
                    }
                    if ui.button("Вернуть пример").clicked() {
                        self.template_json = example_package_json();
                        self.input_json = example_input_json();
                        self.result_json.clear();
                        self.status = "Загружен встроенный пример; он не является клиническим нормативом."
                            .to_owned();
                    }
                });

                if !self.result_json.is_empty() {
                    ui.label(egui::RichText::new("Результат evaluation API").strong());
                    ui.add(
                        egui::TextEdit::multiline(&mut self.result_json)
                            .code_editor()
                            .desired_rows(18)
                            .desired_width(f32::INFINITY)
                            .interactive(false),
                    );
                }
            });
    }

    fn load_file(&mut self) {
        let path = PathBuf::from(self.path.trim());
        match fs::read_to_string(&path) {
            Ok(content) => match TemplatePackage::from_json(&content) {
                Ok(_) => {
                    self.template_json = content;
                    self.original_json = self.template_json.clone();
                    self.result_json.clear();
                    self.status = format!("JSON загружен и валиден: {}", path.display());
                }
                Err(error) => {
                    self.status = format!(
                        "Файл прочитан, но не активирован из-за ошибки валидации: {error}"
                    );
                }
            },
            Err(error) => {
                self.status = format!("Не удалось прочитать {}: {error}", path.display());
            }
        }
    }

    fn validate_current(&mut self) {
        match TemplatePackage::from_json(&self.template_json) {
            Ok(package) => {
                self.status = format!(
                    "Пакет валиден: {} {} · полей {} · вычислений {} · правил {}",
                    package.package.name,
                    package.package.version,
                    package.fields.len(),
                    package.calculations.len(),
                    package.rules.len()
                );
            }
            Err(error) => self.status = format!("Ошибка проверки: {error}"),
        }
    }

    fn format_current(&mut self) {
        match TemplatePackage::from_json(&self.template_json)
            .and_then(|package| package.to_pretty_json())
        {
            Ok(formatted) => {
                self.template_json = formatted;
                self.status = "JSON отформатирован после успешной валидации".to_owned();
            }
            Err(error) => self.status = format!("Нельзя форматировать: {error}"),
        }
    }

    fn save_current(&mut self) {
        let package = match TemplatePackage::from_json(&self.template_json) {
            Ok(package) => package,
            Err(error) => {
                self.status = format!("Сохранение отменено: {error}");
                return;
            }
        };
        let content = match package.to_pretty_json() {
            Ok(content) => content,
            Err(error) => {
                self.status = format!("Сохранение отменено: {error}");
                return;
            }
        };
        let path = PathBuf::from(self.path.trim());
        if let Some(parent) = path.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                self.status = format!("Не удалось создать {}: {error}", parent.display());
                return;
            }
        }
        match fs::write(&path, &content) {
            Ok(()) => {
                self.template_json = content.clone();
                self.original_json = content;
                self.status = format!("Проверенный пакет сохранён: {}", path.display());
            }
            Err(error) => {
                self.status = format!("Не удалось сохранить {}: {error}", path.display());
            }
        }
    }

    fn evaluate_current(&mut self) {
        let package = match TemplatePackage::from_json(&self.template_json) {
            Ok(package) => package,
            Err(error) => {
                self.status = format!("Вычисление отменено: {error}");
                return;
            }
        };
        let input: Value = match serde_json::from_str(&self.input_json) {
            Ok(input) => input,
            Err(error) => {
                self.status = format!("Ошибка тестового контекста JSON: {error}");
                return;
            }
        };
        let result = package.evaluate(&input);
        match serde_json::to_string_pretty(&result) {
            Ok(json) => {
                let error_count = result.errors.len();
                self.result_json = json;
                self.status = if error_count == 0 {
                    format!(
                        "Вычисление завершено: сработало правил {}",
                        result.activated_rules.len()
                    )
                } else {
                    format!(
                        "Вычисление завершено с диагностикой: ошибок {}",
                        error_count
                    )
                };
            }
            Err(error) => self.status = format!("Не удалось сериализовать результат: {error}"),
        }
    }
}

pub fn example_package_json() -> String {
    serde_json::to_string_pretty(&example_package()).expect("built-in clinical example must serialize")
}

fn example_input_json() -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "patient": {
            "age": 67,
            "sex": "male"
        },
        "vitals": {
            "height_cm": 176,
            "weight_kg": 92
        },
        "labs": {
            "creatinine_umol_l": 185
        }
    }))
    .expect("built-in clinical input must serialize")
}

fn example_package() -> TemplatePackage {
    let source = SourceReference {
        document: "DEMO ONLY — replace with current order/clinical guideline".to_owned(),
        section: None,
        effective_date: None,
        note: Some(
            "Пример демонстрирует механику. Перед клиническим применением правило должно быть сверено с актуальным источником."
                .to_owned(),
        ),
    };

    TemplatePackage {
        schema_version: TEMPLATE_SCHEMA_VERSION,
        package: PackageMetadata {
            id: "ru.respanso.demo.dynamic-exam".to_owned(),
            name: "Демонстрационный динамический осмотр".to_owned(),
            version: "0.1.0".to_owned(),
            status: "draft".to_owned(),
            description: "Рост/вес → ИМТ; креатинин/возраст/пол → рСКФ; правила по результатам."
                .to_owned(),
        },
        fields: vec![
            field("patient.age", "Возраст", FieldKind::Integer, None, true),
            choice_field("patient.sex", "Пол", &["male", "female"], true),
            field(
                "vitals.height_cm",
                "Рост",
                FieldKind::Decimal,
                Some("cm"),
                true,
            ),
            field(
                "vitals.weight_kg",
                "Вес",
                FieldKind::Decimal,
                Some("kg"),
                true,
            ),
            field(
                "vitals.bmi",
                "ИМТ",
                FieldKind::Computed,
                Some("kg/m2"),
                false,
            ),
            field(
                "labs.creatinine_umol_l",
                "Креатинин",
                FieldKind::Decimal,
                Some("umol/L"),
                true,
            ),
            field(
                "renal.egfr",
                "рСКФ CKD-EPI 2021",
                FieldKind::Computed,
                Some("mL/min/1.73m2"),
                false,
            ),
            field(
                "renal.review_flag",
                "Нужна клиническая проверка почечного блока",
                FieldKind::Boolean,
                None,
                false,
            ),
        ],
        calculations: vec![
            Calculation {
                id: "calc.bmi".to_owned(),
                output: "vitals.bmi".to_owned(),
                kind: CalculationKind::Builtin {
                    builtin: BuiltinCalculator::Bmi,
                },
                inputs: BTreeMap::from([
                    ("height_cm".to_owned(), "vitals.height_cm".to_owned()),
                    ("weight_kg".to_owned(), "vitals.weight_kg".to_owned()),
                ]),
                precision: Some(1),
                source: None,
            },
            Calculation {
                id: "calc.egfr".to_owned(),
                output: "renal.egfr".to_owned(),
                kind: CalculationKind::Builtin {
                    builtin: BuiltinCalculator::CkdEpi2021,
                },
                inputs: BTreeMap::from([
                    (
                        "creatinine_umol_l".to_owned(),
                        "labs.creatinine_umol_l".to_owned(),
                    ),
                    ("age".to_owned(), "patient.age".to_owned()),
                    ("sex".to_owned(), "patient.sex".to_owned()),
                ]),
                precision: Some(0),
                source: Some(source.clone()),
            },
        ],
        rules: vec![
            Rule {
                id: "demo.egfr_lt_60".to_owned(),
                when: Condition::Lt {
                    left: Operand::Variable {
                        var: "renal.egfr".to_owned(),
                    },
                    right: Operand::Literal {
                        value: Value::from(60),
                    },
                },
                actions: vec![
                    Action::Set {
                        field: "renal.review_flag".to_owned(),
                        value: Operand::Literal {
                            value: Value::Bool(true),
                        },
                    },
                    Action::Warning {
                        message: "рСКФ {{renal.egfr}}: проверить почечный блок по актуальному нормативному источнику."
                            .to_owned(),
                        level: Some("review".to_owned()),
                    },
                ],
                source: Some(source.clone()),
            },
            Rule {
                id: "demo.egfr_lt_30".to_owned(),
                when: Condition::Lt {
                    left: Operand::Variable {
                        var: "renal.egfr".to_owned(),
                    },
                    right: Operand::Literal {
                        value: Value::from(30),
                    },
                },
                actions: vec![Action::Recommendation {
                    message:
                        "Проверить показания к консультации нефролога по актуальной версии рекомендаций."
                            .to_owned(),
                }],
                source: Some(source),
            },
        ],
        sections: BTreeMap::from([
            (
                "objective_status".to_owned(),
                "Рост {{vitals.height_cm}} см, вес {{vitals.weight_kg}} кг, ИМТ {{vitals.bmi}} кг/м²."
                    .to_owned(),
            ),
            (
                "investigations".to_owned(),
                "Креатинин {{labs.creatinine_umol_l}} мкмоль/л, рСКФ {{renal.egfr}} мл/мин/1,73 м²."
                    .to_owned(),
            ),
            ("recommendations".to_owned(), String::new()),
        ]),
    }
}

fn field(
    id: &str,
    label: &str,
    kind: FieldKind,
    unit: Option<&str>,
    required: bool,
) -> TemplateField {
    TemplateField {
        id: id.to_owned(),
        label: label.to_owned(),
        kind,
        unit: unit.map(ToOwned::to_owned),
        required,
        default: None,
        choices: Vec::new(),
        source: None,
    }
}

fn choice_field(id: &str, label: &str, choices: &[&str], required: bool) -> TemplateField {
    TemplateField {
        id: id.to_owned(),
        label: label.to_owned(),
        kind: FieldKind::Choice,
        unit: None,
        required,
        default: None,
        choices: choices.iter().map(|choice| (*choice).to_owned()).collect(),
        source: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_package_validates_and_roundtrips() {
        let json = example_package_json();
        let package = TemplatePackage::from_json(&json).unwrap();
        assert_eq!(package.schema_version, TEMPLATE_SCHEMA_VERSION);
        assert_eq!(TemplatePackage::from_json(&package.to_pretty_json().unwrap()).unwrap(), package);
    }

    #[test]
    fn bmi_is_calculated_from_named_fields() {
        let package = example_package();
        let result = package.evaluate(&serde_json::json!({
            "patient": {"age": 40, "sex": "male"},
            "vitals": {"height_cm": 180, "weight_kg": 81},
            "labs": {"creatinine_umol_l": 90}
        }));
        assert_eq!(result.values.get("vitals.bmi").and_then(Value::as_f64), Some(25.0));
    }

    #[test]
    fn egfr_rules_are_dependency_driven() {
        let package = example_package();
        let result = package.evaluate(&serde_json::json!({
            "patient": {"age": 67, "sex": "male"},
            "vitals": {"height_cm": 176, "weight_kg": 92},
            "labs": {"creatinine_umol_l": 185}
        }));
        assert!(result.values.contains_key("renal.egfr"));
        assert!(result
            .activated_rules
            .iter()
            .any(|rule| rule == "demo.egfr_lt_60"));
        assert_eq!(
            result.values.get("renal.review_flag"),
            Some(&Value::Bool(true))
        );
    }

    #[test]
    fn unknown_rule_field_is_rejected() {
        let mut package = example_package();
        package.rules.push(Rule {
            id: "bad".to_owned(),
            when: Condition::Exists {
                var: "missing.field".to_owned(),
            },
            actions: Vec::new(),
            source: None,
        });
        assert!(package.validate().is_err());
    }

    #[test]
    fn rhai_formula_uses_only_declared_inputs() {
        let calculation = Calculation {
            id: "calc.test".to_owned(),
            output: "out".to_owned(),
            kind: CalculationKind::Formula {
                expression: "a * 2 + b".to_owned(),
            },
            inputs: BTreeMap::from([
                ("a".to_owned(), "a".to_owned()),
                ("b".to_owned(), "b".to_owned()),
            ]),
            precision: Some(0),
            source: None,
        };
        let values = BTreeMap::from([
            ("a".to_owned(), Value::from(3)),
            ("b".to_owned(), Value::from(4)),
        ]);
        assert_eq!(evaluate_calculation(&calculation, &values).unwrap(), Value::from(10.0));
    }
}
