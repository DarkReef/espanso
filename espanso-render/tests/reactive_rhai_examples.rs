use std::path::PathBuf;

use rhai::{Dynamic, Engine, Map, Scope, FLOAT};

fn test_engine() -> Engine {
    let mut engine = Engine::new();
    engine.register_fn("round", |value: FLOAT| value.round());
    engine.disable_symbol("eval");
    engine.disable_symbol("import");
    engine
}

fn modules_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("espanso-render must be inside the repository root")
        .join("examples/medical-calculators/modules")
}

fn form_input(fields: &[(&str, &str)]) -> Dynamic {
    let mut input = Map::new();
    for (name, value) in fields {
        input.insert((*name).into(), Dynamic::from((*value).to_owned()));
    }
    Dynamic::from_map(input)
}

fn calculate(module: &str, fields: &[(&str, &str)]) -> Map {
    let engine = test_engine();
    let path = modules_root().join(module);
    let ast = engine
        .compile_file(path.clone())
        .unwrap_or_else(|error| panic!("unable to compile {}: {error}", path.display()));
    let output = engine
        .call_fn::<Dynamic>(
            &mut Scope::new(),
            &ast,
            "calculate",
            (form_input(fields),),
        )
        .unwrap_or_else(|error| panic!("unable to execute {}: {error}", path.display()));

    output
        .try_cast::<Map>()
        .unwrap_or_else(|| panic!("{} must return a map", path.display()))
}

fn text_field(result: &Map, name: &str) -> String {
    result
        .get(name)
        .unwrap_or_else(|| panic!("result is missing '{name}'"))
        .to_string()
}

#[test]
fn medical_calculators_compile_and_return_reactive_preview_contract() {
    let cases: Vec<(&str, Vec<(&str, &str)>)> = vec![
        (
            "score2.rhai",
            vec![
                ("age", "55"),
                ("sex", "Мужчина"),
                ("region", "Очень высокий риск (Россия)"),
                ("current_smoker", "Нет"),
                ("sbp", "140"),
                ("total_cholesterol", "5,5"),
                ("hdl_cholesterol", "1,3"),
                ("established_cvd", "Нет"),
                ("diabetes", "Нет"),
                ("special_high_risk", "Нет"),
            ],
        ),
        (
            "score2-op.rhai",
            vec![
                ("age", "75"),
                ("sex", "Мужчина"),
                ("region", "Очень высокий риск (Россия)"),
                ("current_smoker", "Нет"),
                ("diabetes", "Нет"),
                ("sbp", "150"),
                ("total_cholesterol", "6,0"),
                ("hdl_cholesterol", "1,4"),
                ("established_cvd", "Нет"),
                ("special_high_risk", "Нет"),
            ],
        ),
        (
            "has-bled.rhai",
            vec![
                ("age", "70"),
                ("hypertension", "Нет"),
                ("renal", "Нет"),
                ("liver", "Нет"),
                ("stroke", "Нет"),
                ("bleeding", "Нет"),
                ("labile_inr", "Нет / не применимо"),
                ("drugs", "Нет"),
                ("alcohol", "Нет"),
            ],
        ),
        (
            "cha2ds2-vasc.rhai",
            vec![
                ("age", "70"),
                ("sex", "Мужчина"),
                ("heart_failure", "Нет"),
                ("hypertension", "Нет"),
                ("diabetes", "Нет"),
                ("prior_stroke", "Нет"),
                ("vascular_disease", "Нет"),
            ],
        ),
        (
            "findrisc.rhai",
            vec![
                ("age", "50"),
                ("sex", "Мужчина"),
                ("weight", "80"),
                ("height", "175"),
                ("waist", "94"),
                ("physical_activity", "Да"),
                ("daily_vegetables", "Да"),
                ("bp_medication", "Нет"),
                ("high_glucose", "Нет"),
                ("family_history", "Нет"),
            ],
        ),
    ];

    for (module, fields) in cases {
        let result = calculate(module, &fields);
        assert_eq!(text_field(&result, "status"), "ok", "module: {module}");
        assert!(result.get("value").is_some(), "module: {module}");
        assert!(
            !text_field(&result, "text").trim().is_empty(),
            "module: {module}"
        );
    }
}

#[test]
fn invalid_numeric_input_returns_error_map_instead_of_aborting_preview() {
    let result = calculate(
        "score2.rhai",
        &[
            ("age", "не число"),
            ("sex", "Мужчина"),
            ("region", "Очень высокий риск (Россия)"),
            ("current_smoker", "Нет"),
            ("sbp", "140"),
            ("total_cholesterol", "5,5"),
            ("hdl_cholesterol", "1,3"),
            ("established_cvd", "Нет"),
            ("diabetes", "Нет"),
            ("special_high_risk", "Нет"),
        ],
    );

    assert_eq!(text_field(&result, "status"), "error");
    assert_eq!(text_field(&result, "value"), "");
    assert!(text_field(&result, "text").contains("целым числом"));
}
