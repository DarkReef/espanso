use std::path::PathBuf;

use rhai::{Dynamic, Engine, Map, Scope};

fn run_calculator(module: &str, fields: &[(&str, &str)]) -> String {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../examples/medical-calculators/modules")
        .join(module);
    let engine = Engine::new();
    let ast = engine
        .compile_file(script.clone())
        .unwrap_or_else(|error| panic!("failed to compile {}: {error}", script.display()));

    let mut input = Map::new();
    for (name, value) in fields {
        input.insert((*name).into(), Dynamic::from((*value).to_owned()));
    }

    engine
        .call_fn::<Dynamic>(
            &mut Scope::new(),
            &ast,
            "calculate",
            (Dynamic::from_map(input),),
        )
        .unwrap_or_else(|error| panic!("failed to execute {}: {error}", script.display()))
        .to_string()
}

fn score2_fields<'a>(sex: &'a str, region: &'a str) -> Vec<(&'static str, &'a str)> {
    vec![
        ("age", "50"),
        ("sex", sex),
        ("region", region),
        ("current_smoker", "Да"),
        ("sbp", "140"),
        ("total_cholesterol", "5.5"),
        ("hdl_cholesterol", "1.3"),
        ("established_cvd", "Нет"),
        ("diabetes", "Нет"),
        ("special_high_risk", "Нет"),
    ]
}

#[test]
fn score2_matches_published_reference_examples() {
    let male_low = run_calculator("score2.rhai", &score2_fields("Мужчина", "Низкий риск"));
    let male_very_high = run_calculator(
        "score2.rhai",
        &score2_fields("Мужчина", "Очень высокий риск (Россия)"),
    );
    let female_low = run_calculator("score2.rhai", &score2_fields("Женщина", "Низкий риск"));
    let female_very_high = run_calculator(
        "score2.rhai",
        &score2_fields("Женщина", "Очень высокий риск (Россия)"),
    );

    assert!(male_low.contains("SCORE2: 5.9%"), "{male_low}");
    assert!(male_very_high.contains("SCORE2: 14"), "{male_very_high}");
    assert!(female_low.contains("SCORE2: 4.2%"), "{female_low}");
    assert!(female_very_high.contains("SCORE2: 13.7%"), "{female_very_high}");
}

#[test]
fn score2_rejects_out_of_scope_diabetes() {
    let mut fields = score2_fields("Мужчина", "Очень высокий риск (Россия)");
    fields
        .iter_mut()
        .find(|(name, _)| *name == "diabetes")
        .expect("diabetes field should exist")
        .1 = "Да";

    let result = run_calculator("score2.rhai", &fields);
    assert!(result.contains("SCORE2 не применяется"), "{result}");
    assert!(result.contains("сахарный диабет"), "{result}");
}

#[test]
fn score2_op_continuous_equation_reference_cases() {
    let base = [
        ("age", "75"),
        ("sex", "Мужчина"),
        ("region", "Низкий риск"),
        ("current_smoker", "Нет"),
        ("diabetes", "Нет"),
        ("sbp", "150"),
        ("total_cholesterol", "6.0"),
        ("hdl_cholesterol", "1.4"),
        ("established_cvd", "Нет"),
        ("special_high_risk", "Нет"),
    ];
    let low = run_calculator("score2-op.rhai", &base);

    let mut very_high = base;
    very_high[2].1 = "Очень высокий риск (Россия)";
    let very_high = run_calculator("score2-op.rhai", &very_high);

    assert!(low.contains("SCORE2-OP: 14.9%"), "{low}");
    assert!(very_high.contains("SCORE2-OP: 35.5%"), "{very_high}");
}

#[test]
fn has_bled_reaches_the_defined_maximum() {
    let result = run_calculator(
        "has-bled.rhai",
        &[
            ("age", "66"),
            ("hypertension", "Да"),
            ("renal", "Да"),
            ("liver", "Да"),
            ("stroke", "Да"),
            ("bleeding", "Да"),
            ("labile_inr", "Да"),
            ("drugs", "Да"),
            ("alcohol", "Да"),
        ],
    );

    assert!(result.contains("HAS-BLED: 9 из 9"), "{result}");
    assert!(result.contains("не использовать балл"), "{result}");
}

#[test]
fn cha2ds2_vasc_and_va_reach_their_defined_maxima() {
    let result = run_calculator(
        "cha2ds2-vasc.rhai",
        &[
            ("age", "75"),
            ("sex", "Женщина"),
            ("heart_failure", "Да"),
            ("hypertension", "Да"),
            ("diabetes", "Да"),
            ("prior_stroke", "Да"),
            ("vascular_disease", "Да"),
        ],
    );

    assert!(result.contains("CHA₂DS₂-VASc: 9 из 9"), "{result}");
    assert!(result.contains("CHA₂DS₂-VA: 8 из 8"), "{result}");
}

#[test]
fn findrisc_handles_minimum_and_maximum_profiles() {
    let minimum = run_calculator(
        "findrisc.rhai",
        &[
            ("age", "40"),
            ("sex", "Мужчина"),
            ("weight", "65"),
            ("height", "175"),
            ("waist", "85"),
            ("physical_activity", "Да"),
            ("daily_vegetables", "Да"),
            ("bp_medication", "Нет"),
            ("high_glucose", "Нет"),
            ("family_history", "Нет"),
        ],
    );
    let maximum = run_calculator(
        "findrisc.rhai",
        &[
            ("age", "65"),
            ("sex", "Женщина"),
            ("weight", "100"),
            ("height", "160"),
            ("waist", "100"),
            ("physical_activity", "Нет"),
            ("daily_vegetables", "Нет"),
            ("bp_medication", "Да"),
            ("high_glucose", "Да"),
            ("family_history", "Родители, брат/сестра или ребёнок"),
        ],
    );

    assert!(minimum.contains("FINDRISC: 0 из 26"), "{minimum}");
    assert!(maximum.contains("FINDRISC: 26 из 26"), "{maximum}");
}
