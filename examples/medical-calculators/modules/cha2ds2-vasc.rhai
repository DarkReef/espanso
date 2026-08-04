// CHA2DS2-VASc (classic) and CHA2DS2-VA (ESC 2024) for clinical atrial fibrillation.
// Module version: 1.1.0 (2026-08-04). Reactive-preview result contract.

fn point(value) {
    if value == "Да" { 1 } else { 0 }
}

fn parse_integer(value) {
    let normalized = value;
    normalized.trim();
    parse_int(normalized)
}

fn fail(message) {
    #{
        status: "error",
        value: "",
        text: message
    }
}

fn calculate(input) {
    if input.age.trim() == "" {
        return fail("Укажите возраст пациента.");
    }

    let age = 0;
    try {
        age = parse_integer(input.age);
    } catch {
        return fail("Возраст должен быть целым числом.");
    }
    if age < 18 || age > 120 {
        return fail("CHA₂DS₂-VASc не рассчитан.\n\nПроверьте возраст пациента.");
    }

    let heart_failure = point(input.heart_failure);
    let hypertension = point(input.hypertension);
    let diabetes = point(input.diabetes);
    let prior_stroke = point(input.prior_stroke);
    let vascular_disease = point(input.vascular_disease);
    let age_points = if age >= 75 { 2 } else if age >= 65 { 1 } else { 0 };
    let sex_points = if input.sex == "Женщина" { 1 } else { 0 };

    let score_va = heart_failure + hypertension + age_points + diabetes + 2 * prior_stroke + vascular_disease;
    let score_vasc = score_va + sex_points;

    let recommendation = if score_va >= 2 {
        "CHA₂DS₂-VA ≥2: антикоагуляция рекомендована при отсутствии противопоказаний"
    } else if score_va == 1 {
        "CHA₂DS₂-VA =1: антикоагуляцию следует рассмотреть совместно с пациентом"
    } else {
        "CHA₂DS₂-VA =0: низкий балльный риск; оцените иные клинические факторы и пересматривайте риск"
    };

    let components = "";
    if heart_failure == 1 { components += "C — сердечная недостаточность: 1;\n"; }
    if hypertension == 1 { components += "H — артериальная гипертензия: 1;\n"; }
    if age_points == 2 { components += "A₂ — возраст ≥75 лет: 2;\n"; }
    if age_points == 1 { components += "A — возраст 65–74 года: 1;\n"; }
    if diabetes == 1 { components += "D — сахарный диабет: 1;\n"; }
    if prior_stroke == 1 { components += "S₂ — инсульт/ТИА/артериальная эмболия: 2;\n"; }
    if vascular_disease == 1 { components += "V — сосудистое заболевание: 1;\n"; }
    if sex_points == 1 { components += "Sc — женский пол: 1 только в классическом CHA₂DS₂-VASc;\n"; }
    if components == "" { components = "Балльных факторов не отмечено.\n"; }

    let text = `CHA₂DS₂-VASc: ${score_vasc} из 9\nCHA₂DS₂-VA: ${score_va} из 8\n\n${recommendation}\n\nКомпоненты:\n${components}\nESC 2024 предлагает CHA₂DS₂-VA без полового компонента для поддержки решения об антикоагуляции. Калькулятор предназначен для подтверждённой клинической ФП.\n\nМодуль v1.1.0. Результат поддерживает, но не заменяет клиническое решение.`;

    #{
        status: "ok",
        value: score_va,
        score_va: score_va,
        score_vasc: score_vasc,
        text: text
    }
}
