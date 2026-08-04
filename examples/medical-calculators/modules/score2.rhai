// SCORE2 continuous equation with sex-specific coefficients and regional recalibration.
// Source: SCORE2 Working Group, Eur Heart J. 2021;42:2439-2454.
// DOI: 10.1093/eurheartj/ehab309; four-decimal parameters: ehab761.
// Module version: 1.1.0 (2026-08-04). Reactive-preview result contract.

fn yes(value) {
    value == "Да"
}

fn parse_number(value) {
    let normalized = value.trim().replace(",", ".");
    parse_float(normalized)
}

fn fail(message) {
    #{
        status: "error",
        value: "",
        text: message
    }
}

fn risk_category(age, risk_percent) {
    if age < 50 {
        if risk_percent < 2.5 {
            "Низкий–умеренный риск"
        } else if risk_percent < 7.5 {
            "Высокий риск"
        } else {
            "Очень высокий риск"
        }
    } else if risk_percent < 5.0 {
        "Низкий–умеренный риск"
    } else if risk_percent < 10.0 {
        "Высокий риск"
    } else {
        "Очень высокий риск"
    }
}

fn calculate(input) {
    if input.age.trim() == "" {
        return fail("Укажите возраст пациента.");
    }
    if input.sbp.trim() == "" {
        return fail("Укажите систолическое артериальное давление.");
    }
    if input.total_cholesterol.trim() == "" {
        return fail("Укажите общий холестерин.");
    }
    if input.hdl_cholesterol.trim() == "" {
        return fail("Укажите ХС-ЛПВП.");
    }

    let age = 0;
    try {
        age = parse_int(input.age.trim());
    } catch {
        return fail("Возраст должен быть целым числом.");
    }

    let age_value = 0.0;
    try {
        age_value = parse_number(input.age);
    } catch {
        return fail("Возраст должен быть числом.");
    }

    let sbp = 0.0;
    try {
        sbp = parse_number(input.sbp);
    } catch {
        return fail("САД должно быть числом.");
    }

    let total_cholesterol = 0.0;
    try {
        total_cholesterol = parse_number(input.total_cholesterol);
    } catch {
        return fail("Общий холестерин должен быть числом.");
    }

    let hdl_cholesterol = 0.0;
    try {
        hdl_cholesterol = parse_number(input.hdl_cholesterol);
    } catch {
        return fail("ХС-ЛПВП должен быть числом.");
    }

    if age < 40 || age > 69 {
        return fail("SCORE2 не рассчитан.\n\nДопустимый возраст: 40–69 лет. Для возраста ≥70 лет используйте SCORE2-OP.");
    }
    if sbp < 80.0 || sbp > 250.0 {
        return fail("SCORE2 не рассчитан.\n\nПроверьте САД: ожидается значение 80–250 мм рт. ст.");
    }
    if total_cholesterol < 1.0 || total_cholesterol > 20.0 {
        return fail("SCORE2 не рассчитан.\n\nПроверьте общий холестерин: ожидается значение 1–20 ммоль/л.");
    }
    if hdl_cholesterol < 0.2 || hdl_cholesterol > 5.0 {
        return fail("SCORE2 не рассчитан.\n\nПроверьте ХС-ЛПВП: ожидается значение 0,2–5 ммоль/л.");
    }

    let exclusion = "";
    if yes(input.established_cvd) {
        exclusion += "• документированное сердечно-сосудистое заболевание;\n";
    }
    if yes(input.diabetes) {
        exclusion += "• сахарный диабет;\n";
    }
    if yes(input.special_high_risk) {
        exclusion += "• СГХС, умеренная/тяжёлая ХБП или другое заведомо высокорисковое состояние;\n";
    }
    if exclusion != "" {
        return fail(`SCORE2 не применяется к указанному пациенту.\n\n${exclusion}\nИспользуйте профильную стратификацию риска и клинические рекомендации.`);
    }

    let male = input.sex == "Мужчина";
    let smoker = if yes(input.current_smoker) { 1.0 } else { 0.0 };
    let cage = (age_value - 60.0) / 5.0;
    let csbp = (sbp - 120.0) / 20.0;
    let ctchol = total_cholesterol - 6.0;
    let chdl = (hdl_cholesterol - 1.3) / 0.5;

    let lp = 0.0;
    let baseline_survival = 0.0;
    let scale1 = 0.0;
    let scale2 = 0.0;

    if male {
        lp = 0.3742 * cage;
        lp += 0.6012 * smoker;
        lp += 0.2777 * csbp;
        lp += 0.1458 * ctchol;
        lp -= 0.2698 * chdl;
        lp -= 0.0755 * cage * smoker;
        lp -= 0.0255 * cage * csbp;
        lp -= 0.0281 * cage * ctchol;
        lp += 0.0426 * cage * chdl;
        baseline_survival = 0.9605;

        if input.region == "Низкий риск" {
            scale1 = -0.5699;
            scale2 = 0.7476;
        } else if input.region == "Умеренный риск" {
            scale1 = -0.1565;
            scale2 = 0.8009;
        } else if input.region == "Высокий риск" {
            scale1 = 0.3207;
            scale2 = 0.9360;
        } else {
            scale1 = 0.5836;
            scale2 = 0.8294;
        }
    } else {
        lp = 0.4648 * cage;
        lp += 0.7744 * smoker;
        lp += 0.3131 * csbp;
        lp += 0.1002 * ctchol;
        lp -= 0.2606 * chdl;
        lp -= 0.1088 * cage * smoker;
        lp -= 0.0277 * cage * csbp;
        lp -= 0.0226 * cage * ctchol;
        lp += 0.0613 * cage * chdl;
        baseline_survival = 0.9776;

        if input.region == "Низкий риск" {
            scale1 = -0.7380;
            scale2 = 0.7019;
        } else if input.region == "Умеренный риск" {
            scale1 = -0.3143;
            scale2 = 0.7701;
        } else if input.region == "Высокий риск" {
            scale1 = 0.5710;
            scale2 = 0.9369;
        } else {
            scale1 = 0.9412;
            scale2 = 0.8329;
        }
    }

    let uncalibrated = 1.0 - baseline_survival ** exp(lp);
    let cumulative_hazard = -ln(1.0 - uncalibrated);
    let log_cumulative_hazard = ln(cumulative_hazard);
    let recalibrated_log_hazard = scale1 + scale2 * log_cumulative_hazard;
    let calibrated = 1.0 - exp(-exp(recalibrated_log_hazard));
    let risk_percent = round(calibrated * 1000.0) / 10.0;
    let category = risk_category(age, risk_percent);
    let text = `SCORE2: ${risk_percent}%\nКатегория: ${category}\nГоризонт: 10 лет, фатальные и нефатальные СС-события\nРегион: ${input.region}\n\nВозраст: ${age} лет; пол: ${input.sex}\nСАД: ${sbp} мм рт. ст.; курение: ${input.current_smoker}\nОХС: ${total_cholesterol} ммоль/л; ХС-ЛПВП: ${hdl_cholesterol} ммоль/л\n\nМодуль v1.1.0. Результат поддерживает, но не заменяет клиническое решение.`;

    #{
        status: "ok",
        value: risk_percent,
        category: category,
        text: text
    }
}
