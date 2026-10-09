// Модифицированная шкала EHRA. Контракт reactive preview unstable.

fn calculate(input) {
    let selected = input.ehra_class;
    let text = if selected.starts_with("IIa") {
        "EHRA: IIa (лёгкие симптомы, повседневная активность не нарушена)"
    } else if selected.starts_with("IIb") {
        "EHRA: IIb (симптомы беспокоят, повседневная активность не нарушена)"
    } else if selected.starts_with("III") {
        "EHRA: III (выраженные симптомы, повседневная активность нарушена)"
    } else if selected.starts_with("IV") {
        "EHRA: IV (инвалидизирующие симптомы, повседневная активность невозможна)"
    } else {
        "EHRA: I (симптомов, связанных с ФП, нет)"
    };
    #{ status: "ok", value: text, text: text }
}
