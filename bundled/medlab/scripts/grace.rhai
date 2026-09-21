// GRACE 1.0: госпитальный риск при ОКС. Контракт reactive preview unstable.

fn yes(value) { value == "Да" }
fn fail(message) { #{ status: "error", value: "", text: message } }
fn parse_number(value) { let text = value; text.trim(); text.replace(",", "."); parse_float(text) }
fn age_points(age) { if age < 30.0 { 0 } else if age < 40.0 { 8 } else if age < 50.0 { 25 } else if age < 60.0 { 41 } else if age < 70.0 { 58 } else if age < 80.0 { 75 } else if age < 90.0 { 91 } else { 100 } }
fn heart_rate_points(hr) { if hr < 50.0 { 0 } else if hr < 70.0 { 3 } else if hr < 90.0 { 9 } else if hr < 110.0 { 15 } else if hr < 150.0 { 24 } else if hr < 200.0 { 38 } else { 46 } }
fn sbp_points(sbp) { if sbp < 80.0 { 58 } else if sbp < 100.0 { 53 } else if sbp < 120.0 { 43 } else if sbp < 140.0 { 34 } else if sbp < 160.0 { 24 } else if sbp < 200.0 { 10 } else { 0 } }
fn creatinine_points(creatinine_umol) { let mg_dl = creatinine_umol / 88.4; if mg_dl < 0.4 { 1 } else if mg_dl < 0.8 { 4 } else if mg_dl < 1.2 { 7 } else if mg_dl < 1.6 { 10 } else if mg_dl < 2.0 { 13 } else if mg_dl < 4.0 { 21 } else { 28 } }
fn killip_points(killip) { if killip == "I" { 0 } else if killip == "II" { 20 } else if killip == "III" { 39 } else { 59 } }

fn calculate(input) {
    if input.ecg_type == "Подъём ST или эквивалент" {
        let text = "Подозрение на ОКС с подъёмом ST или эквивалент. GRACE не определяет срочность реперфузии: показаны немедленная экстренная госпитализация и маршрутизация в ЧКВ-центр.";
        return #{ status: "ok", value: "STEMI", text: text };
    }

    let age = 0.0;
    let hr = 0.0;
    let sbp = 0.0;
    let creatinine = 0.0;
    try { age = parse_number(input.age); } catch { return fail("Возраст должен быть числом."); }
    try { hr = parse_number(input.heart_rate); } catch { return fail("ЧСС должна быть числом."); }
    try { sbp = parse_number(input.systolic_bp); } catch { return fail("САД должно быть числом."); }
    try { creatinine = parse_number(input.creatinine); } catch { return fail("Креатинин должен быть числом."); }
    if age < 18.0 || age > 120.0 || hr < 20.0 || hr > 300.0 || sbp < 40.0 || sbp > 300.0 || creatinine < 10.0 || creatinine > 2000.0 {
        return fail("Проверьте возраст, ЧСС, САД и креатинин.");
    }

    let score = age_points(age) + heart_rate_points(hr) + sbp_points(sbp) + creatinine_points(creatinine) + killip_points(input.killip);
    if yes(input.cardiac_arrest) { score += 39; }
    if yes(input.st_deviation) { score += 28; }
    if yes(input.troponin) { score += 14; }

    let risk = if score <= 108 { "низкий госпитальный риск" } else if score <= 140 { "промежуточный госпитальный риск" } else { "высокий госпитальный риск" };
    let strategy = if yes(input.very_high_risk) {
        "Показаны экстренная госпитализация в ЧКВ-центр и немедленная инвазивная стратегия, ориентировочно в течение 2 часов"
    } else if score > 140 || yes(input.confirmed_nstemi) || yes(input.st_deviation) {
        "Показаны экстренная госпитализация в ЧКВ-центр и ранняя инвазивная стратегия в течение 24 часов"
    } else {
        "Показана экстренная госпитализация; дальнейшая инвазивная стратегия определяется динамикой ЭКГ, тропонина и клинического состояния"
    };
    let text = `ОКС без подъёма ST; GRACE 1.0: ${score} баллов (${risk}). ${strategy}.`;
    #{ status: "ok", value: score, category: risk, text: text }
}
