// SCORE2-OP continuous equation with sex-specific coefficients and regional recalibration.
// Source: SCORE2-OP Working Group, Eur Heart J. 2021;42:2455-2467.
// DOI: 10.1093/eurheartj/ehab312.
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

fn risk_category(risk_percent) {
    if risk_percent < 7.5 {
        "Низкий–умеренный риск"
    } else if risk_percent < 15.0 {
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

    let age = try {
        parse_int(input.age.trim())
    } catch {
        return fail("Возраст должен быть целым числом.");
    };
    let age_value = try {
        parse_number(input.age)
    } catch {
        return fail("Возраст должен быть числом.");
    };
    let sbp = try {
        parse_number(input.sbp)
    } catch {
        return fail("САД должно быть числом.");
    };
    let total_cholesterol = try {
        parse_number(input.total_cholesterol)
    } catch {
        return fail("Общий холестерин должен быть числом.");
    };
    let hdl_cholesterol = try {
        parse_number(input.hdl_cholesterol)
    } catch {
        return fail("ХС-ЛПВП должен быть числом.");
    };

    if age < 70 || age > 89 {
        return fail("SCORE2-OP не рассчитан.\n\nПоддерживаемый возраст: 70–89 лет. Для возраста 40–69 лет используйте SCORE2.");
    }
    if sbp < 80.0 || sbp > 250.0 {
        return fail("SCORE2-OP не рассчитан.\n\nПроверьте САД: ожидается значение 80–250 мм рт. ст.");
    }
    if total_cholesterol < 1.0 || total_cholesterol > 20.0 {
        return fail("SCORE2-OP не рассчитан.\n\nПроверьте общий холестерин: ожидается значение 1–20 ммоль/л.");
    }
    if hdl_cholesterol < 0.2 || hdl_cholesterol > 5.0 {
        return fail("SCORE2-OP не рассчитан.\n\nПроверьте ХС-ЛПВП: ожидается значение 0,2–5 ммоль/л.");
    }
    if yes(input.established_cvd) {
        return fail("SCORE2-OP не применяется при документированном сердечно-сосудистом заболевании.\n\nИспользуйте вторичную профилактику и профильные рекомендации.");
    }
    if yes(input.special_high_risk) {
        return fail("SCORE2-OP не применяется как основной классификатор при СГХС, умеренной/тяжёлой ХБП или другом заведомо высокорисковом состоянии.");
    }

    let male = input.sex == "Мужчина";
    let smoker = if yes(input.current_smoker) { 1.0 } else { 0.0 };
    let diabetes = if yes(input.diabetes) { 1.0 } else { 0.0 };
    let ca = age_value - 73.0;
    let centered_sbp = sbp - 150.0;
    let centered_total = total_cholesterol - 6.0;
    let centered_hdl = hdl_cholesterol - 1.4;

    let lp = 0.0;
    let uncalibrated = 0.0;
    let scale1 = 0.0;
    let scale2 = 0.0;

    if male {
        lp = 0.0634 * ca;
        lp += 0.4245 * diabetes;
        lp += 0.3524 * smoker;
        lp += 0.0094 * centered_sbp;
        lp += 0.0850 * centered_total;
        lp -= 0.3564 * centered_hdl;
        lp -= 0.0174 * ca * diabetes;
        lp -= 0.0247 * ca * smoker;
        lp -= 0.0005 * ca * centered_sbp;
        lp += 0.0073 * ca * centered_total;
        lp += 0.0091 * ca * centered_hdl;
        uncalibrated = 1.0 - 0.7576 ** exp(lp - 0.0929);

        if input.region == "Низкий риск" {
            scale1 = -0.34;
            scale2 = 1.19;
        } else if input.region == "Умеренный риск" {
            scale1 = 0.01;
            scale2 = 1.25;
        } else if input.region == "Высокий риск" {
            scale1 = 0.08;
            scale2 = 1.15;
        } else {
            scale1 = 0.05;
            scale2 = 0.70;
        }
    } else {
        lp = 0.0789 * ca;
        lp += 0.6010 * diabetes;
        lp += 0.4921 * smoker;
        lp += 0.0102 * centered_sbp;
        lp += 0.0605 * centered_total;
        lp -= 0.3040 * centered_hdl;
        lp -= 0.0107 * ca * diabetes;
        lp -= 0.0255 * ca * smoker;
        lp -= 0.0004 * ca * centered_sbp;
        lp -= 0.0009 * ca * centered_total;
        lp += 0.0154 * ca * centered_hdl;
        uncalibrated = 1.0 - 0.8082 ** exp(lp - 0.2290);

        if input.region == "Низкий риск" {
            scale1 = -0.52;
            scale2 = 1.01;
        } else if input.region == "Умеренный риск" {
            scale1 = -0.10;
            scale2 = 1.10;
        } else if input.region == "Высокий риск" {
            scale1 = 0.38;
            scale2 = 1.09;
        } else {
            scale1 = 0.38;
            scale2 = 0.69;
        }
    }

    let cumulative_hazard = -ln(1.0 - uncalibrated);
    let log_cumulative_hazard = ln(cumulative_hazard);
    let recalibrated_log_hazard = scale1 + scale2 * log_cumulative_hazard;
    let calibrated = 1.0 - exp(-exp(recalibrated_log_hazard));
    let risk_percent = round(calibrated * 1000.0) / 10.0;
    let category = risk_category(risk_percent);
    let diabetes_note = if diabetes > 0.0 {
        "\n\nВнимание: диабет включён в исходное уравнение SCORE2-OP, но профилактическую тактику следует сверять с диабет-специфическими рекомендациями."
    } else {
        ""
    };
    let text = `SCORE2-OP: ${risk_percent}%\nКатегория: ${category}\nГоризонт: 10 лет, фатальные и нефатальные СС-события\nРегион: ${input.region}\n\nВозраст: ${age} лет; пол: ${input.sex}\nСАД: ${sbp} мм рт. ст.; курение: ${input.current_smoker}; диабет: ${input.diabetes}\nОХС: ${total_cholesterol} ммоль/л; ХС-ЛПВП: ${hdl_cholesterol} ммоль/л${diabetes_note}\n\nМодуль v1.1.0. Результат поддерживает, но не заменяет клиническое решение.`;

    #{
        status: "ok",
        value: risk_percent,
        category: category,
        text: text
    }
}
