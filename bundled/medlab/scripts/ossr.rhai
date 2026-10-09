// ОССР для возраста 21–39 лет. Контракт reactive preview unstable.

fn fail(message) { #{ status: "error", value: "", text: message } }
fn parse_integer(value) { let text = value; text.trim(); parse_int(text) }

fn calculate(input) {
    let age = 0;
    let sbp = 0;
    let cholesterol = 0;
    try { age = parse_integer(input.age); } catch { return fail("Возраст должен быть целым числом."); }
    try { sbp = parse_integer(input.sbp); } catch { return fail("Проверьте САД."); }
    try { cholesterol = parse_integer(input.total_cholesterol); } catch { return fail("Проверьте общий холестерин."); }
    if age < 21 || age > 39 { return fail("ОССР применяется в возрасте 21–39 лет."); }

    let smoker = input.current_smoker == "Да";
    let risk = 1;
    if !smoker {
        if sbp == 120 { risk = if cholesterol <= 6 { 1 } else { 2 }; }
        else if sbp == 140 {
            risk = if cholesterol == 4 { 1 } else if cholesterol <= 7 { 2 } else { 3 };
        } else if sbp == 160 {
            risk = if cholesterol == 4 { 2 } else if cholesterol <= 6 { 3 } else { 4 };
        } else {
            risk = if cholesterol <= 5 { 3 } else if cholesterol == 6 { 4 } else if cholesterol == 7 { 5 } else { 6 };
        }
    } else if sbp == 120 {
        risk = if cholesterol <= 5 { 2 } else if cholesterol <= 7 { 3 } else { 4 };
    } else if sbp == 140 {
        risk = if cholesterol <= 5 { 3 } else if cholesterol == 6 { 4 } else if cholesterol == 7 { 5 } else { 6 };
    } else if sbp == 160 {
        risk = if cholesterol == 4 { 4 } else if cholesterol == 5 { 5 } else if cholesterol == 6 { 6 } else if cholesterol == 7 { 7 } else { 8 };
    } else {
        risk = if cholesterol == 4 { 6 } else if cholesterol == 5 { 7 } else if cholesterol == 6 { 8 } else if cholesterol == 7 { 10 } else { 12 };
    }

    let category = if risk > 1 { "высокий относительный сердечно-сосудистый риск" } else { "низкий относительный сердечно-сосудистый риск" };
    #{ status: "ok", value: risk, category: category, text: `ОССР: ${risk} (${category})` }
}
