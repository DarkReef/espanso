// Finnish Diabetes Risk Score (FINDRISC), original 8-item non-laboratory score.
// Source: Lindstrom J, Tuomilehto J. Diabetes Care. 2003;26:725-731.
// DOI: 10.2337/diacare.26.3.725.
// Module version: 1.1.0 (2026-08-04). Reactive-preview result contract.

fn yes(value) {
    value == "Да"
}

fn parse_number(value) {
    let normalized = value.trim();
    normalized.replace(",", ".");
    parse_float(normalized)
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
    if input.weight.trim() == "" {
        return fail("Укажите массу тела.");
    }
    if input.height.trim() == "" {
        return fail("Укажите рост.");
    }
    if input.waist.trim() == "" {
        return fail("Укажите окружность талии.");
    }

    let age = 0;
    try {
        age = parse_int(input.age.trim());
    } catch {
        return fail("Возраст должен быть целым числом.");
    }

    let weight = 0.0;
    try {
        weight = parse_number(input.weight);
    } catch {
        return fail("Масса тела должна быть числом.");
    }

    let height_cm = 0.0;
    try {
        height_cm = parse_number(input.height);
    } catch {
        return fail("Рост должен быть числом.");
    }

    let waist = 0.0;
    try {
        waist = parse_number(input.waist);
    } catch {
        return fail("Окружность талии должна быть числом.");
    }

    if age < 18 || age > 120 {
        return fail("FINDRISC не рассчитан.\n\nПроверьте возраст пациента.");
    }
    if weight < 30.0 || weight > 350.0 {
        return fail("FINDRISC не рассчитан.\n\nПроверьте массу тела: ожидается значение 30–350 кг.");
    }
    if height_cm < 100.0 || height_cm > 230.0 {
        return fail("FINDRISC не рассчитан.\n\nПроверьте рост: ожидается значение 100–230 см.");
    }
    if waist < 40.0 || waist > 220.0 {
        return fail("FINDRISC не рассчитан.\n\nПроверьте окружность талии: ожидается значение 40–220 см.");
    }

    let height_m = height_cm / 100.0;
    let bmi = weight / (height_m * height_m);
    let bmi_display = round(bmi * 10.0) / 10.0;

    let age_points = if age < 45 { 0 } else if age <= 54 { 2 } else if age <= 64 { 3 } else { 4 };
    let bmi_points = if bmi < 25.0 { 0 } else if bmi < 30.0 { 1 } else { 3 };

    let waist_points = 0;
    if input.sex == "Мужчина" {
        if waist < 94.0 {
            waist_points = 0;
        } else if waist <= 102.0 {
            waist_points = 3;
        } else {
            waist_points = 4;
        }
    } else if waist < 80.0 {
        waist_points = 0;
    } else if waist <= 88.0 {
        waist_points = 3;
    } else {
        waist_points = 4;
    }

    let activity_points = if yes(input.physical_activity) { 0 } else { 2 };
    let vegetables_points = if yes(input.daily_vegetables) { 0 } else { 1 };
    let bp_medication_points = if yes(input.bp_medication) { 2 } else { 0 };
    let high_glucose_points = if yes(input.high_glucose) { 5 } else { 0 };
    let family_points = if input.family_history == "Нет" {
        0
    } else if input.family_history == "Только родственники 2-й степени" {
        3
    } else {
        5
    };

    let score = age_points;
    score += bmi_points;
    score += waist_points;
    score += activity_points;
    score += vegetables_points;
    score += bp_medication_points;
    score += high_glucose_points;
    score += family_points;

    let interpretation = if score < 7 {
        "Низкий риск: примерно 1 из 100"
    } else if score <= 11 {
        "Слегка повышенный риск: примерно 1 из 25"
    } else if score <= 14 {
        "Умеренный риск: примерно 1 из 6"
    } else if score <= 20 {
        "Высокий риск: примерно 1 из 3"
    } else {
        "Очень высокий риск: примерно 1 из 2"
    };

    let cohort_note = if age < 35 || age > 64 {
        "\n\nВнимание: исходная проспективная разработка проводилась преимущественно в возрасте 35–64 лет; интерпретируйте результат осторожно."
    } else {
        ""
    };
    let text = `FINDRISC: ${score} из 26\n${interpretation} риска СД 2 типа в течение 10 лет\n\nИМТ: ${bmi_display} кг/м²; талия: ${waist} см\nВозраст: ${age}; пол: ${input.sex}\nФизическая активность ≥30 мин/день: ${input.physical_activity}\nОвощи/фрукты/ягоды ежедневно: ${input.daily_vegetables}\nАнтигипертензивные препараты: ${input.bp_medication}\nГипергликемия ранее: ${input.high_glucose}\nСемейный анамнез: ${input.family_history}${cohort_note}\n\nМодуль v1.1.0. Скрининговый результат не устанавливает диагноз.`;

    #{
        status: "ok",
        value: score,
        bmi: bmi_display,
        category: interpretation,
        text: text
    }
}
