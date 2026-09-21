// Формулировка ХБП с расчётом рСКФ CKD-EPI 2021.
// При наличии четвёртого аргумента он трактуется как АКМ/ACR в мг/г, не как альбумин мочи в мг/л.

fn calc(sex, age, creatinine) {
    let scr = creatinine / 88.4;
    let kappa = if sex == "ж" { 0.7 } else { 0.9 };
    let alpha = if sex == "ж" { -0.241 } else { -0.302 };
    let sex_factor = if sex == "ж" { 1.012 } else { 1.0 };
    let ratio = scr / kappa;
    let minimum = if ratio < 1.0 { ratio } else { 1.0 };
    let maximum = if ratio > 1.0 { ratio } else { 1.0 };

    142.0
        * minimum ** alpha
        * maximum ** (-1.2)
        * 0.9938 ** age
        * sex_factor
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

fn albuminuria(acr) {
    if acr < 30.0 {
        "A1"
    } else if acr <= 300.0 {
        "A2"
    } else {
        "A3"
    }
}

fn calculate(input) {
    let parts = input.split("|");
    if parts.len < 3 {
        return "ХБП: ошибка данных";
    }

    let sex = parts[0];
    let age = parse_float(parts[1]);
    let creatinine = parse_float(parts[2]);

    if (sex != "м" && sex != "ж") || age < 18.0 || age > 120.0 || creatinine <= 0.0 {
        return "ХБП: проверьте пол, возраст и креатинин";
    }

    let value = round(calc(sex, age, creatinine) * 10.0) / 10.0;
    let base = `Хроническая болезнь почек, стадия ${stage(value)} (рСКФ CKD-EPI 2021 ${value} мл/мин/1,73 м²)`;

    if parts.len >= 4 && parts[3] != "" {
        let acr = parse_float(parts[3]);
        if acr < 0.0 {
            return "ХБП: проверьте АКМ";
        }
        `${base}, категория альбуминурии ${albuminuria(acr)} (АКМ ${acr} мг/г).`
    } else {
        `${base}, степень альбуминурии требует уточнения.`
    }
}
