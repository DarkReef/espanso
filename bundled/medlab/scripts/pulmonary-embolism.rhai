// ТЭЛА: Wells, revised Geneva и sPESI. Контракт reactive preview unstable.

fn yes(value) { value == "Да" }
fn fail(message) { #{ status: "error", value: "", text: message } }
fn parse_number(value) { let text = value; text.trim(); text.replace(",", "."); parse_float(text) }

fn calculate(input) {
    let age = 0.0;
    let hr = 0.0;
    let sbp = 0.0;
    let spo2 = 0.0;
    try { age = parse_number(input.age); } catch { return fail("ТЭЛА: расчёт не выполнен — проверьте возраст"); }
    try { hr = parse_number(input.heart_rate); } catch { return fail("ТЭЛА: расчёт не выполнен — проверьте ЧСС"); }
    try { sbp = parse_number(input.systolic_bp); } catch { return fail("ТЭЛА: расчёт не выполнен — проверьте САД"); }
    try { spo2 = parse_number(input.oxygen_saturation); } catch { return fail("ТЭЛА: расчёт не выполнен — проверьте SpO₂"); }
    if age < 18.0 || age > 120.0 { return fail("ТЭЛА: расчёт не выполнен — проверьте возраст"); }
    if hr < 20.0 || hr > 300.0 || sbp < 40.0 || sbp > 300.0 || spo2 < 30.0 || spo2 > 100.0 {
        return fail("ТЭЛА: расчёт не выполнен — проверьте ЧСС, САД и SpO₂");
    }

    let wells = 0.0;
    if yes(input.dvt_signs) { wells += 3.0; }
    if yes(input.pe_more_likely) { wells += 3.0; }
    if hr > 100.0 { wells += 1.5; }
    if yes(input.immobilization_surgery) { wells += 1.5; }
    if yes(input.previous_vte) { wells += 1.5; }
    if yes(input.hemoptysis) { wells += 1.0; }
    if yes(input.active_cancer) { wells += 1.0; }
    wells = round(wells * 10.0) / 10.0;
    let wells_text = if wells > 4.0 { "ТЭЛА вероятна" } else { "ТЭЛА маловероятна" };

    let geneva = 0;
    if age > 65.0 { geneva += 1; }
    if yes(input.previous_vte) { geneva += 3; }
    if yes(input.surgery_fracture_month) { geneva += 2; }
    if yes(input.active_cancer) { geneva += 2; }
    if yes(input.unilateral_leg_pain) { geneva += 3; }
    if yes(input.hemoptysis) { geneva += 2; }
    if hr >= 95.0 { geneva += 5; } else if hr >= 75.0 { geneva += 3; }
    if yes(input.deep_vein_pain_edema) { geneva += 4; }
    let geneva_text = if geneva <= 3 { "низкая вероятность" } else if geneva <= 10 { "промежуточная вероятность" } else { "высокая вероятность" };

    let text = `Wells: ${wells} балла (${wells_text}); Geneva: ${geneva} баллов (${geneva_text}). `;
    if yes(input.pregnancy) {
        text += "Шкалы Wells/Geneva не валидированы для беременности и послеродового периода; требуется экстренная специализированная оценка.";
    } else if yes(input.confirmed_pe) {
        let spesi = 0;
        if age > 80.0 { spesi += 1; }
        if yes(input.active_cancer) { spesi += 1; }
        if yes(input.chronic_hf) || yes(input.chronic_lung_disease) { spesi += 1; }
        if hr >= 110.0 { spesi += 1; }
        if sbp < 100.0 { spesi += 1; }
        if spo2 < 90.0 { spesi += 1; }
        let spesi_text = if spesi == 0 { "низкий риск раннего неблагоприятного исхода" } else { "повышенный риск раннего неблагоприятного исхода" };
        text += `sPESI: ${spesi} баллов (${spesi_text}). Подтверждённая ТЭЛА требует срочной специализированной оценки и стратификации риска по гемодинамике и функции правого желудочка.`;
    } else if wells > 4.0 || geneva >= 6 {
        text += "Показана экстренная госпитализация и визуализация лёгочных артерий; отрицательный D-димер не должен задерживать обследование при высокой клинической вероятности.";
    } else {
        text += "Показана экстренная клиническая оценка; при низкой или промежуточной вероятности следующим этапом обычно является D-димер с последующей визуализацией по результату.";
    }

    #{ status: "ok", value: wells, wells: wells, geneva: geneva, text: text }
}
