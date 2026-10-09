// Cockcroft–Gault, креатинин в мкмоль/л. Контракт reactive preview unstable.

fn fail(message) { #{ status: "error", value: "", text: message } }
fn parse_number(value) { let text = value; text.trim(); text.replace(",", "."); parse_float(text) }

fn calculate(input) {
    let age = 0.0;
    let weight = 0.0;
    let creatinine = 0.0;
    try { age = parse_number(input.age); } catch { return fail("Возраст должен быть числом."); }
    try { weight = parse_number(input.weight); } catch { return fail("Масса тела должна быть числом."); }
    try { creatinine = parse_number(input.creatinine); } catch { return fail("Креатинин должен быть числом."); }
    if age < 18.0 || age > 120.0 || weight < 20.0 || weight > 350.0 || creatinine < 20.0 || creatinine > 3000.0 {
        return fail("Проверьте возраст, массу тела и креатинин.");
    }

    let crcl = (140.0 - age) * weight / (0.814 * creatinine);
    if input.sex == "Женщина" { crcl *= 0.85; }
    crcl = round(crcl * 10.0) / 10.0;
    let text = `Клиренс креатинина по Cockcroft–Gault: ${crcl} мл/мин (${input.weight_type}: ${weight} кг). Это CrCl, а не рСКФ в мл/мин/1,73 м²; выбор массы и пороги дозирования следует сверять с инструкцией препарата.`;
    #{ status: "ok", value: crcl, text: text }
}
