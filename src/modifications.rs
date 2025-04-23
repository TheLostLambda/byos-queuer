// Standard Library Imports
use std::io::Read;

// External Crate Imports
use color_eyre::Result;
use serde_json::Value;

// Public API ==========================================================================================================

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Modifications(String);

impl Modifications {
    pub fn from_txt(mut txt: impl Read) -> Result<Self> {
        let mut modifications = String::new();
        txt.read_to_string(&mut modifications)?;

        Ok(Self(modifications))
    }

    #[must_use]
    pub fn to_json(&self) -> Value {
        let modifications = self.0.clone();

        Value::String(modifications)
    }
}

// Unit Tests ==========================================================================================================

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use indoc::indoc;
    use serde_json::json;

    use super::*;

    const TXT: &[u8] = indoc! {b"
        % Custom modification text below
        HexNAc(1)MurNAc_alditol(1) @ NTerm | common1
        HexN(1) MurNAc_alditol(1) @ NTerm | common1
        HexNAc(1)MurNAc_alditol(1) / -20.0262 @ NTerm | common1
        HexN(1) MurNAc_alditol(1) / -20.0262 @ NTerm | common1
        HexNAc(1)MurNAc_alditol(1) / +42.0106 @ NTerm | common1

        cleavage_flags=0
        "};

    static MODIFICATIONS: LazyLock<Modifications> = LazyLock::new(|| {
        Modifications("% Custom modification text below\nHexNAc(1)MurNAc_alditol(1) @ NTerm | common1\nHexN(1) MurNAc_alditol(1) @ NTerm | common1\nHexNAc(1)MurNAc_alditol(1) / -20.0262 @ NTerm | common1\nHexN(1) MurNAc_alditol(1) / -20.0262 @ NTerm | common1\nHexNAc(1)MurNAc_alditol(1) / +42.0106 @ NTerm | common1\n\ncleavage_flags=0\n".to_owned())
    });

    #[test]
    fn from_txt() {
        let expected = Modifications("% Custom modification text below\nHexNAc(1)MurNAc_alditol(1) @ NTerm | common1\nHexN(1) MurNAc_alditol(1) @ NTerm | common1\nHexNAc(1)MurNAc_alditol(1) / -20.0262 @ NTerm | common1\nHexN(1) MurNAc_alditol(1) / -20.0262 @ NTerm | common1\nHexNAc(1)MurNAc_alditol(1) / +42.0106 @ NTerm | common1\n\ncleavage_flags=0\n".to_owned());

        let modifications = Modifications::from_txt(TXT);
        assert!(modifications.is_ok());

        assert_eq!(modifications.unwrap(), expected);
    }

    #[test]
    fn to_json() {
        let expected = json!("% Custom modification text below\nHexNAc(1)MurNAc_alditol(1) @ NTerm | common1\nHexN(1) MurNAc_alditol(1) @ NTerm | common1\nHexNAc(1)MurNAc_alditol(1) / -20.0262 @ NTerm | common1\nHexN(1) MurNAc_alditol(1) / -20.0262 @ NTerm | common1\nHexNAc(1)MurNAc_alditol(1) / +42.0106 @ NTerm | common1\n\ncleavage_flags=0\n");

        let json = MODIFICATIONS.to_json();

        assert_eq!(json, expected);
    }
}
