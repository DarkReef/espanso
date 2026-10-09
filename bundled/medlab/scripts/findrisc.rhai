// FINDRISC: 10-летний риск СД 2 типа. Контракт reactive preview unstable.

fn yes(value) { value == "Да" }
fn fail(message) { #{ status: "error", value: "", text: message } }
fn parse_integer(value) { let text = value; text.trim(); parse_int(text) }
fn parse_number(value) { let text = value; text.trim(); text.replace(",", "."); parse_float(text) }

fn calculate(input) {
    let age = 0;
    let waist = 0.0;
    try { age = parse_integer(input.age); } catch { return fail("Возраст должен быть целым числом."); }
    try { waist = parse_number(input.waist); } catch { return fail("Окружность талии должна быть числом."); }
    if age < 18 || age > 120 { return fail("Проверьте возраст пациента."); }
    if waist < 40.0 || waist > 220.0 { return fail("Проверьте окружность талии: ожидается 40–220 см."); }

    let score = 0;
    if age >= 65 { score += 4; } else if age >= 55 { score += 3; } else if age >= 45 { score += 2; }
    if input.bmi == "25–30" { score += 1; } else if input.bmi == ">30" { score += 3; }

    if input.sex == "Мужчина" {
        if waist > 102.0 { score += 4; } else if waist >= 94.0 { score += 3; }
    } else {
        if waist > 88.0 { score += 4; } else if waist >= 80.0 { score += 3; }
    }

    if !yes(input.physical_activity) { score += 2; }
    if !yes(input.vegetables_daily) { score += 1; }
    if yes(input.antihypertensive) { score += 2; }
    if yes(input.high_glucose) { score += 5; }
    if input.family_diabetes == "Да: родитель, брат/сестра или ребёнок" { score += 5; }
    else if input.family_diabetes != "Нет" { score += 3; }

    let interpretation = if score < 7 { "низкий риск СД2, около 1% за 10 лет" }
        else if score < 12 { "слегка повышенный риск СД2, около 4% за 10 лет" }
        else if score < 15 { "умеренный риск СД2, около 17% за 10 лет" }
        else if score < 21 { "высокий риск СД2, около 33% за 10 лет" }
        else { "очень высокий риск СД2, около 50% за 10 лет" };

    #{ status: "ok", value: score, category: interpretation, text: `FINDRISC: ${score} баллов (${interpretation})` }
}
