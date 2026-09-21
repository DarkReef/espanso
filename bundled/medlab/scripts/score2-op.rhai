// SCORE2 / SCORE2-OP с региональной рекалибровкой.
// SCORE2: Eur Heart J. 2021;42:2439–2454; DOI 10.1093/eurheartj/ehab309.
// SCORE2-OP: Eur Heart J. 2021;42:2455–2467; DOI 10.1093/eurheartj/ehab312.
// Контракт rEspanso v1.0 Unstable: #{ status, value, text, ... }.

fn yes(value) {
    value == "Да"
}

fn fail(message) {
    #{ status: "error", value: "", text: message }
}

fn parse_integer(value) {
    let normalized = value;
    normalized.trim();
    parse_int(normalized)
}

fn parse_number(value) {
    let normalized = value;
    normalized.trim();
    normalized.replace(",", ".");
    parse_float(normalized)
}

fn forced_very_high(input) {
    yes(input.established_cvd) || yes(input.diabetes) || yes(input.special_high_risk)
}

fn score2_category(age, risk_percent) {
    if age < 50 {
        if risk_percent < 2.5 { "низкий–умеренный риск" }
        else if risk_percent < 7.5 { "высокий риск" }
        else { "очень высокий риск" }
    } else if risk_percent < 5.0 {
        "низкий–умеренный риск"
    } else if risk_percent < 10.0 {
        "высокий риск"
    } else {
        "очень высокий риск"
    }
}

fn score2op_category(risk_percent) {
    if risk_percent < 7.5 { "низкий–умеренный риск" }
    else if risk_percent < 15.0 { "высокий риск" }
    else { "очень высокий риск" }
}

fn calculate_score2(input, age, age_value, sbp, total_cholesterol, hdl_cholesterol) {
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
        lp = 0.3742 * cage + 0.6012 * smoker + 0.2777 * csbp + 0.1458 * ctchol - 0.2698 * chdl;
        lp -= 0.0755 * cage * smoker + 0.0255 * cage * csbp + 0.0281 * cage * ctchol;
        lp += 0.0426 * cage * chdl;
        baseline_survival = 0.9605;
        if input.region == "Низкий риск" { scale1 = -0.5699; scale2 = 0.7476; }
        else if input.region == "Умеренный риск" { scale1 = -0.1565; scale2 = 0.8009; }
        else if input.region == "Высокий риск" { scale1 = 0.3207; scale2 = 0.9360; }
        else { scale1 = 0.5836; scale2 = 0.8294; }
    } else {
        lp = 0.4648 * cage + 0.7744 * smoker + 0.3131 * csbp + 0.1002 * ctchol - 0.2606 * chdl;
        lp -= 0.1088 * cage * smoker + 0.0277 * cage * csbp + 0.0226 * cage * ctchol;
        lp += 0.0613 * cage * chdl;
        baseline_survival = 0.9776;
        if input.region == "Низкий риск" { scale1 = -0.7380; scale2 = 0.7019; }
        else if input.region == "Умеренный риск" { scale1 = -0.3143; scale2 = 0.7701; }
        else if input.region == "Высокий риск" { scale1 = 0.5710; scale2 = 0.9369; }
        else { scale1 = 0.9412; scale2 = 0.8329; }
    }

    let uncalibrated = 1.0 - baseline_survival ** exp(lp);
    let log_hazard = ln(-ln(1.0 - uncalibrated));
    let calibrated = 1.0 - exp(-exp(scale1 + scale2 * log_hazard));
    let risk_percent = round(calibrated * 1000.0) / 10.0;
    let category = if forced_very_high(input) { "очень высокий риск" } else { score2_category(age, risk_percent) };
    #{ model: "SCORE-2", value: risk_percent, category: category }
}

fn calculate_score2op(input, age_value, sbp, total_cholesterol, hdl_cholesterol) {
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
        lp = 0.0634 * ca + 0.4245 * diabetes + 0.3524 * smoker + 0.0094 * centered_sbp;
        lp += 0.0850 * centered_total - 0.3564 * centered_hdl - 0.0174 * ca * diabetes;
        lp -= 0.0247 * ca * smoker + 0.0005 * ca * centered_sbp;
        lp += 0.0073 * ca * centered_total + 0.0091 * ca * centered_hdl;
        uncalibrated = 1.0 - 0.7576 ** exp(lp - 0.0929);
        if input.region == "Низкий риск" { scale1 = -0.34; scale2 = 1.19; }
        else if input.region == "Умеренный риск" { scale1 = 0.01; scale2 = 1.25; }
        else if input.region == "Высокий риск" { scale1 = 0.08; scale2 = 1.15; }
        else { scale1 = 0.05; scale2 = 0.70; }
    } else {
        lp = 0.0789 * ca + 0.6010 * diabetes + 0.4921 * smoker + 0.0102 * centered_sbp;
        lp += 0.0605 * centered_total - 0.3040 * centered_hdl - 0.0107 * ca * diabetes;
        lp -= 0.0255 * ca * smoker + 0.0004 * ca * centered_sbp + 0.0009 * ca * centered_total;
        lp += 0.0154 * ca * centered_hdl;
        uncalibrated = 1.0 - 0.8082 ** exp(lp - 0.2290);
        if input.region == "Низкий риск" { scale1 = -0.52; scale2 = 1.01; }
        else if input.region == "Умеренный риск" { scale1 = -0.10; scale2 = 1.10; }
        else if input.region == "Высокий риск" { scale1 = 0.38; scale2 = 1.09; }
        else { scale1 = 0.38; scale2 = 0.69; }
    }

    let log_hazard = ln(-ln(1.0 - uncalibrated));
    let calibrated = 1.0 - exp(-exp(scale1 + scale2 * log_hazard));
    let risk_percent = round(calibrated * 1000.0) / 10.0;
    let category = if forced_very_high(input) { "очень высокий риск" } else { score2op_category(risk_percent) };
    #{ model: "SCORE-2-OP", value: risk_percent, category: category }
}

fn calculate(input) {
    if input.age.trim() == "" { return fail("Укажите возраст."); }
    if input.sbp.trim() == "" { return fail("Укажите систолическое артериальное давление."); }
    if input.total_cholesterol.trim() == "" { return fail("Укажите общий холестерин."); }

    let age = 0;
    try { age = parse_integer(input.age); } catch { return fail("Возраст должен быть целым числом."); }
    if age < 40 || age > 89 { return fail("SCORE2/SCORE2-OP применяется в возрасте 40–89 лет."); }

    let age_value = 0.0;
    let sbp = 0.0;
    let total_cholesterol = 0.0;
    try { age_value = parse_number(input.age); } catch { return fail("Проверьте возраст."); }
    try { sbp = parse_number(input.sbp); } catch { return fail("САД должно быть числом."); }
    try { total_cholesterol = parse_number(input.total_cholesterol); } catch { return fail("Общий холестерин должен быть числом."); }

    let hdl_text = input.hdl_cholesterol;
    hdl_text.trim();
    let hdl_assumed = hdl_text == "" || hdl_text == "-";
    let hdl_cholesterol = 1.0;
    if !hdl_assumed {
        try { hdl_cholesterol = parse_number(hdl_text); } catch { return fail("ХС-ЛПВП должен быть числом."); }
    }

    if sbp < 80.0 || sbp > 250.0 { return fail("Проверьте САД: ожидается 80–250 мм рт. ст."); }
    if total_cholesterol < 1.0 || total_cholesterol > 20.0 { return fail("Проверьте общий холестерин: ожидается 1–20 ммоль/л."); }
    if hdl_cholesterol < 0.2 || hdl_cholesterol > 5.0 { return fail("Проверьте ХС-ЛПВП: ожидается 0,2–5 ммоль/л."); }

    let result = if age <= 69 {
        calculate_score2(input, age, age_value, sbp, total_cholesterol, hdl_cholesterol)
    } else {
        calculate_score2op(input, age_value, sbp, total_cholesterol, hdl_cholesterol)
    };

    let text = `${result.model}: ${result.value}% (${result.category})`;
    if hdl_assumed { text += ". ХС-ЛПВП не указан: в расчёте принято 1,0 ммоль/л"; }

    #{
        status: "ok",
        value: result.value,
        model: result.model,
        category: result.category,
        text: text
    }
}
