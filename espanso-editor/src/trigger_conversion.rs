#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerRegexConversion {
    pub pattern: String,
    pub examples: String,
}

#[must_use]
pub fn from_triggers(triggers: &[String]) -> TriggerRegexConversion {
    let literal = triggers
        .iter()
        .map(String::as_str)
        .filter(|trigger| !trigger.trim().is_empty())
        .collect::<Vec<_>>();
    let escaped = literal
        .iter()
        .map(|trigger| regex::escape(trigger))
        .collect::<Vec<_>>();
    let pattern = match escaped.as_slice() {
        [] => String::new(),
        [trigger] => format!("{trigger}$"),
        _ => format!("(?:{})$", escaped.join("|")),
    };
    TriggerRegexConversion {
        pattern,
        examples: literal.join("\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_single_trigger_to_literal_end_anchored_regexp() {
        let conversion = from_triggers(&[":doc".to_owned()]);
        assert_eq!(conversion.pattern, ":doc$");
        assert_eq!(conversion.examples, ":doc");
        let regex = regex::Regex::new(&conversion.pattern).expect("converted regexp");
        assert!(regex.is_match("текст :doc"));
        assert!(!regex.is_match("текст :doc продолжение"));
    }

    #[test]
    fn converts_multiple_triggers_and_escapes_metacharacters() {
        let conversion = from_triggers(&[":a.b".to_owned(), ":c+".to_owned()]);
        assert_eq!(conversion.pattern, r"(?::a\.b|:c\+)$");
        assert_eq!(conversion.examples, ":a.b\n:c+");
    }

    #[test]
    fn empty_triggers_produce_empty_conversion() {
        let conversion = from_triggers(&[String::new(), "   ".to_owned()]);
        assert!(conversion.pattern.is_empty());
        assert!(conversion.examples.is_empty());
    }
}
