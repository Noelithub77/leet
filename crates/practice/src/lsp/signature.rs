//! Interpret server-provided parameter labels without guessing argument positions.
use std::ops::Range;
use lsp_types::{ParameterLabel, SignatureHelp, SignatureInformation};

pub fn active_signature(help: &SignatureHelp) -> usize {
    help.active_signature.map(|ix| ix as usize).filter(|&ix| ix < help.signatures.len()).unwrap_or(0)
}

pub fn active_parameter<'a>(help: &SignatureHelp, signature: &'a SignatureInformation) -> Option<&'a lsp_types::ParameterInformation> {
    let parameters = signature.parameters.as_ref()?;
    let ix = signature.active_parameter.or(help.active_parameter).unwrap_or(0) as usize;
    parameters.get(ix).or_else(|| parameters.first())
}

fn utf16_offset(label: &str, target: u32) -> Option<usize> {
    let mut units = 0;
    for (offset, ch) in label.char_indices() {
        if units == target { return Some(offset); }
        units += ch.len_utf16() as u32;
    }
    (units == target).then_some(label.len())
}

pub fn parameter_range(help: &SignatureHelp, signature: &SignatureInformation) -> Option<Range<usize>> {
    let parameter = active_parameter(help, signature)?;
    match &parameter.label {
        ParameterLabel::LabelOffsets([start, end]) => {
            let range = utf16_offset(&signature.label, *start)?..utf16_offset(&signature.label, *end)?;
            (range.start < range.end).then_some(range)
        }
        ParameterLabel::Simple(label) => {
            if label.is_empty() { return None; }
            let mut cursor = signature.label.find('(').map_or(0, |offset| offset + 1);
            for candidate in signature.parameters.as_ref()? {
                let range = match &candidate.label {
                    ParameterLabel::Simple(label) => { let start = cursor + signature.label.get(cursor..)?.find(label)?; start..start + label.len() },
                    ParameterLabel::LabelOffsets([start, end]) => utf16_offset(&signature.label, *start)?..utf16_offset(&signature.label, *end)?,
                };
                if std::ptr::eq(candidate, parameter) { return Some(range); }
                cursor = range.end;
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn help(value: serde_json::Value) -> SignatureHelp { serde_json::from_value(value).unwrap() }
    #[test]
    fn signature_active_parameter_overrides_the_response_and_utf16_is_respected() {
        let help = help(serde_json::json!({"activeParameter":0,"signatures":[{"label":"f(😀, value: int)","activeParameter":1,"parameters":[{"label":[2,4]},{"label":[6,16]}]}]}));
        let signature = &help.signatures[0];
        let range = parameter_range(&help, signature).unwrap();
        assert_eq!(&signature.label[range], "value: int");
        let mut malformed = signature.clone(); malformed.parameters.as_mut().unwrap()[1].label = ParameterLabel::LabelOffsets([3,4]);
        assert!(parameter_range(&help, &malformed).is_none());
    }
    #[test]
    fn malformed_indices_and_missing_parameters_have_safe_defaults() {
        let help = help(serde_json::json!({"activeSignature":50,"activeParameter":50,"signatures":[{"label":"f(value)","parameters":[{"label":"value"}]},{"label":"g()"}]}));
        assert_eq!(active_signature(&help), 0);
        assert_eq!(parameter_range(&help, &help.signatures[0]), Some(2..7));
        assert!(parameter_range(&help, &help.signatures[1]).is_none());
    }
    #[test]
    fn repeated_string_labels_highlight_the_selected_parameter_occurrence() {
        let help = help(serde_json::json!({"activeParameter":1,"signatures":[{"label":"value(int, int)","parameters":[{"label":"int"},{"label":"int"}]}]}));
        assert_eq!(parameter_range(&help, &help.signatures[0]), Some(11..14));
    }
}
