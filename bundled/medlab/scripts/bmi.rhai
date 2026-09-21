// ИМТ взрослого: масса (кг) / рост² (м²).
// Категории согласованы с границами ВОЗ для взрослых.

fn values(input) {
    let parts = input.split("|");
    if parts.len != 2 {
        return [0.0, 0.0];
    }
    [parse_float(parts[0]), parse_float(parts[1])]
}

fn bmi_value(input) {
    let data = values(input);
    let height_cm = data[0];
    let weight_kg = data[1];

    if height_cm < 100.0 || height_cm > 250.0 || weight_kg < 20.0 || weight_kg > 500.0 {
        return -1.0;
    }

    let height_m = height_cm / 100.0;
    round(weight_kg / (height_m * height_m) * 100.0) / 100.0
}

fn category_phrase(value) {
    if value < 16.0 {
        "выраженному дефициту массы тела"
    } else if value < 18.5 {
        "недостаточной (дефициту) массе тела"
    } else if value < 25.0 {
        "норме"
    } else if value < 30.0 {
        "избыточной массе тела (предожирению)"
    } else if value < 35.0 {
        "ожирению I степени"
    } else if value < 40.0 {
        "ожирению II степени"
    } else {
        "ожирению III степени"
    }
}

fn diagnosis(value) {
    if value < 16.0 {
        "Выраженный дефицит массы тела"
    } else if value < 18.5 {
        "Недостаточная масса тела"
    } else if value < 25.0 {
        "Нормальная масса тела"
    } else if value < 30.0 {
        "Избыточная масса тела (предожирение)"
    } else if value < 35.0 {
        "Ожирение I степени"
    } else if value < 40.0 {
        "Ожирение II степени"
    } else {
        "Ожирение III степени"
    }
}

fn calculate(input) {
    let value = bmi_value(input);
    if value < 0.0 {
        return "ИМТ: ошибка данных";
    }

    `ИМТ = ${value} кг/м², что соответствует ${category_phrase(value)}.`
}

fn calculate_diagnosis(input) {
    let value = bmi_value(input);
    if value < 0.0 {
        return "Ошибка данных";
    }

    let text = diagnosis(value);
    if value >= 30.0 {
        `${text} (ИМТ = ${value} кг/м²), стадия требует уточнения.`
    } else {
        `${text} (ИМТ = ${value} кг/м²).`
    }
}
