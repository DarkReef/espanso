// CKD-EPI 2021 по креатинину, без расового коэффициента.
// Креатинин вводится в мкмоль/л; формула предназначена для взрослых.

fn calc(sex, age, creatinine) {
    let scr = creatinine / 88.4;
    let kappa = if sex == "f" { 0.7 } else { 0.9 };
    let alpha = if sex == "f" { -0.241 } else { -0.302 };
    let sex_factor = if sex == "f" { 1.012 } else { 1.0 };
    let ratio = scr / kappa;
    let minimum = if ratio < 1.0 { ratio } else { 1.0 };
    let maximum = if ratio > 1.0 { ratio } else { 1.0 };

    142.0
        * minimum ** alpha
        * maximum ** (-1.2)
        * 0.9938 ** age
        * sex_factor
}

fn parse_input(input) {
    input.split("|")
}

fn valid(sex, age, creatinine) {
    (sex == "m" || sex == "f")
        && age >= 18.0
        && age <= 120.0
        && creatinine > 0.0
        && creatinine <= 3000.0
}

fn stage(value) {
    if value >= 90.0 {
        "С1"
    } else if value >= 60.0 {
        "С2"
    } else if value >= 45.0 {
        "С3а"
    } else if value >= 30.0 {
        "С3б"
    } else if value >= 15.0 {
        "С4"
    } else {
        "С5"
    }
}

fn calculate(input) {
    let parts = parse_input(input);
    if parts.len < 3 {
        return "СКФ: ошибка данных";
    }

    let sex = parts[0];
    let age = parse_float(parts[1]);
    let creatinine = parse_float(parts[2]);

    if !valid(sex, age, creatinine) {
        return "СКФ: проверьте пол, возраст и креатинин";
    }

    `${round(calc(sex, age, creatinine))}`
}

fn calculate_diagnosis(input) {
    let parts = parse_input(input);
    if parts.len < 4 {
        return "СКФ: ошибка данных";
    }

    let sex = parts[0];
    let age = parse_float(parts[1]);
    let creatinine = parse_float(parts[2]);
    let date = parts[3];

    if !valid(sex, age, creatinine) {
        return "СКФ: проверьте пол, возраст и креатинин";
    }

    let value = round(calc(sex, age, creatinine));
    `Хроническая болезнь почек, стадия ${stage(value)} (рСКФ CKD-EPI 2021 = ${value} мл/мин/1,73 м², креатинин = ${creatinine} мкмоль/л от ${date}), степень альбуминурии требует уточнения.`
}
