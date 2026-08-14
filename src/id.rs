//! Field and variant identifiers.
//!
//! An identifier is written as a single varint, sometimes followed by a name:
//!
//! | value | meaning |
//! | --- | --- |
//! | below [`ID_LEN`] | the length of the name, which follows |
//! | [`ID_LEN`] | a longer name: its length follows as a varint, then the name |
//! | [`ID_LEN_NAME`] and above | the number of the field, with no name at all |
//!

/// The name length that means the length itself follows.
///
/// Also the first length that does not fit in the single varint, since the
/// values above it name the numbered identifiers.
pub const ID_LEN: usize = 64;

/// The first value that means a numbered identifier rather than a name.
pub const ID_LEN_NAME: usize = ID_LEN + 1;

/// How many identifiers encode as a number.
pub const ID_COUNT: usize = 60;

/// The identifiers that encode as a single byte, `_0` to `_59`.
static NUMBERED_IDENTS: [&str; ID_COUNT] = [
    "_0", "_1", "_2", "_3", "_4", "_5", "_6", "_7", "_8", "_9", "_10", "_11", "_12", "_13", "_14", "_15", "_16",
    "_17", "_18", "_19", "_20", "_21", "_22", "_23", "_24", "_25", "_26", "_27", "_28", "_29", "_30", "_31",
    "_32", "_33", "_34", "_35", "_36", "_37", "_38", "_39", "_40", "_41", "_42", "_43", "_44", "_45", "_46",
    "_47", "_48", "_49", "_50", "_51", "_52", "_53", "_54", "_55", "_56", "_57", "_58", "_59",
];

/// The identifier a numbered field encodes to, if the number fits in one byte.
pub fn numbered_ident(id: usize) -> Option<&'static str> {
    NUMBERED_IDENTS.get(id).copied()
}

/// The number of an identifier of the form `_N`, if it encodes in one byte.
pub fn ident_number(ident: &str) -> Option<usize> {
    let id = ident.strip_prefix("_")?.parse::<usize>().ok()?;
    (numbered_ident(id) == Some(ident)).then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbered_idents_agree() {
        // The writer derives the number from the name and the reader derives
        // the name from the number. They have to be the same mapping.
        for id in 0..ID_COUNT {
            let ident = numbered_ident(id).expect("every number below the count has a name");
            assert_eq!(ident, format!("_{id}"));
            assert_eq!(ident_number(ident), Some(id));
        }

        assert_eq!(numbered_ident(ID_COUNT), None);
        assert_eq!(ident_number(&format!("_{ID_COUNT}")), None);
        assert_eq!(ident_number("name"), None);
        assert_eq!(ident_number("_"), None);
        assert_eq!(ident_number("_x"), None);
    }

    #[test]
    fn only_the_canonical_form_is_numbered() {
        // Anything else would encode as a number that decodes to a different
        // name, and the field would be lost without a word.
        assert_eq!(ident_number("_0"), Some(0));
        assert_eq!(ident_number("_59"), Some(59));

        assert_eq!(ident_number("_00"), None);
        assert_eq!(ident_number("_07"), None);
        assert_eq!(ident_number("_007"), None);
        assert_eq!(ident_number("_+7"), None);
    }
}
