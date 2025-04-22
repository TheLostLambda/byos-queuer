// Standard Library Imports
use std::io::{BufRead, BufReader, Read};

// External Crate Imports
use color_eyre::Result;
use seq_io::fasta::{OwnedRecord, Reader, Record};
use serde_json::{json, Value};

// Public API ==========================================================================================================

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Proteins(Vec<Protein>);

impl Proteins {
    pub fn from_fasta(fasta: impl Read) -> Result<Self> {
        Reader::new(fasta)
            .into_records()
            .collect::<Result<_, _>>()
            .map_err(Into::into)
    }

    pub fn from_txt(txt: impl Read) -> Result<Self> {
        BufReader::new(txt)
            .lines()
            .collect::<Result<_, _>>()
            .map_err(Into::into)
    }

    #[must_use]
    pub fn to_json(&self) -> Value {
        let proteins = self
            .0
            .iter()
            .enumerate()
            .map(|(index, Protein { id, sequence })| {
                json!({
                    "children": [],
                    "data": [
                        id,
                        "",
                        sequence,
                        null,
                        index + 1,
                    ]
                })
            })
            .collect();

        Value::Array(proteins)
    }
}

impl<P: Into<Protein>> FromIterator<P> for Proteins {
    fn from_iter<T: IntoIterator<Item = P>>(iter: T) -> Self {
        Self(iter.into_iter().map(Into::into).collect())
    }
}

// Private Helper Code =================================================================================================

#[derive(Clone, Eq, PartialEq, Debug)]
struct Protein {
    id: String,
    sequence: String,
}

impl Protein {
    fn new(id: impl Into<String>, sequence: impl Into<String>) -> Self {
        let id = id.into();
        let sequence = sequence.into();
        Self { id, sequence }
    }
}

impl From<OwnedRecord> for Protein {
    fn from(value: OwnedRecord) -> Self {
        let id = String::from_utf8_lossy(value.id_bytes());
        let sequence = String::from_utf8_lossy(value.seq());
        Self::new(id, sequence)
    }
}

impl From<String> for Protein {
    fn from(value: String) -> Self {
        Self::new(value.clone(), value)
    }
}

// Unit Tests ==========================================================================================================

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use indoc::indoc;
    use serde_json::json;

    use super::*;

    impl<S1: Into<String>, S2: Into<String>> From<(S1, S2)> for Protein {
        fn from((id, sequence): (S1, S2)) -> Self {
            Self::new(id, sequence)
        }
    }

    const FASTA: &[u8] = indoc! {b"
        >P01013 GENE X PROTEIN (OVALBUMIN-RELATED)
        QIKDLLVSSSTDLDTTLVLVNAIYFKGMWKTAFNAEDTREMPFHVTKQESKPVQMMCMNNSFNVATLPAE
        KMKILELPFASGDLSMLVLLPDEVSDLERIEKTINFEKLTEWTNPNTMEKRRVKVYLPQMKIEEKYNLTS
        VLMALGMTDLFIPSANLTGISSAESLKISQAVHGAFMELSEDGIEMAGSTGVIEDIKHSPESEQFRADHP
        FLFLIKHNPTNTIVYFGRYWSP
        >Z1072 GENE X PROTEIN (HYPE-RELATED)
        MKATKLVLGAVILGSTLLAGCSSNAKIDQLSSDVQTLNAKVDQLSNDVNAMRSDVQAAKDDAARANQRLD
        NMATKYRK
    "};

    const TXT: &[u8] = indoc! {b"
        AEJAA
        AEJA
        AEJ
        AE
    "};

    static PROTEINS: LazyLock<Proteins> = LazyLock::new(|| {
        [
            ("P01013", "ABCDEFG"),
            ("Z10721", "HIJKLMOP"),
            ("F29808", "QRSTUVWXYZ"),
        ]
        .into_iter()
        .collect()
    });

    #[test]
    fn from_fasta() {
        let expected= [
            ("P01013", "QIKDLLVSSSTDLDTTLVLVNAIYFKGMWKTAFNAEDTREMPFHVTKQESKPVQMMCMNNSFNVATLPAEKMKILELPFASGDLSMLVLLPDEVSDLERIEKTINFEKLTEWTNPNTMEKRRVKVYLPQMKIEEKYNLTSVLMALGMTDLFIPSANLTGISSAESLKISQAVHGAFMELSEDGIEMAGSTGVIEDIKHSPESEQFRADHPFLFLIKHNPTNTIVYFGRYWSP"),
            ("Z1072", "MKATKLVLGAVILGSTLLAGCSSNAKIDQLSSDVQTLNAKVDQLSNDVNAMRSDVQAAKDDAARANQRLDNMATKYRK"),
        ].into_iter().collect();

        let proteins = Proteins::from_fasta(FASTA);
        assert!(proteins.is_ok());

        assert_eq!(proteins.unwrap(), expected);
    }

    #[test]
    fn from_txt() {
        let expected = [
            ("AEJAA", "AEJAA"),
            ("AEJA", "AEJA"),
            ("AEJ", "AEJ"),
            ("AE", "AE"),
        ]
        .into_iter()
        .collect();

        let proteins = Proteins::from_txt(TXT);
        assert!(proteins.is_ok());

        assert_eq!(proteins.unwrap(), expected);
    }

    #[test]
    fn to_json() {
        let expected = json!([
           {
               "children": [],
               "data": [
                   "P01013",
                   "",
                   "ABCDEFG",
                   null,
                   1
               ]
           },
           {
               "children": [],
               "data": [
                   "Z10721",
                   "",
                   "HIJKLMOP",
                   null,
                   2
               ]
           },
           {
               "children": [],
               "data": [
                   "F29808",
                   "",
                   "QRSTUVWXYZ",
                   null,
                   3
               ]
           },
        ]);

        let json = PROTEINS.to_json();

        assert_eq!(json, expected);
    }
}
