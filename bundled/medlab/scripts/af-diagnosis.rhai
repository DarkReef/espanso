// Формулировка формы фибрилляции предсердий. Контракт reactive preview unstable.

fn calculate(input) {
    let diagnosis = "";
    if input.af_type == "Впервые диагностированная" {
        diagnosis = "Фибрилляция предсердий, впервые диагностированная";
    } else if input.af_type == "Пароксизмальная" {
        diagnosis = "Фибрилляция предсердий, пароксизмальная форма";
    } else if input.af_type == "Персистирующая" {
        diagnosis = "Фибрилляция предсердий, персистирующая форма";
    } else if input.af_type == "Ранняя персистирующая" {
        diagnosis = "Фибрилляция предсердий, персистирующая форма (ранняя персистирующая)";
    } else if input.af_type == "Длительно персистирующая" {
        diagnosis = "Фибрилляция предсердий, длительно персистирующая форма";
    } else {
        diagnosis = "Фибрилляция предсердий, постоянная форма";
    }

    if input.rate_variant == "Нормосистолический вариант" {
        diagnosis += ", нормосистолический вариант";
    } else if input.rate_variant == "Тахисистолический вариант" {
        diagnosis += ", тахисистолический вариант";
    } else if input.rate_variant == "Брадисистолический вариант" {
        diagnosis += ", брадисистолический вариант";
    }

    #{ status: "ok", value: diagnosis, text: diagnosis }
}
