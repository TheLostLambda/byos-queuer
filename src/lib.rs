// TODO:
// 1) Split `Protein` out into `protein.rs` and serialize to Byos-compatible JSON
// 2) TDD for taking a `State` and writing out a new `.wflw` file
// 3) Queue / command module for collecting `.wflw` files and running them in Byos
// 4) Basic (minimally functional) UI for configuring `State`
// 5) Polish of UI + Skeleton (if time allows!)

use color_eyre::Result;
use std::io::{BufRead, BufReader, Read};

use seq_io::fasta::{OwnedRecord, Reader};

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
        let id = String::from_utf8_lossy(
            &value
                .head
                .into_iter()
                .take_while(|&b| b != b' ')
                .collect::<Vec<_>>(),
        )
        .into_owned();
        let sequence = String::from_utf8_lossy(&value.seq).into_owned();
        Self { id, sequence }
    }
}

impl From<String> for Protein {
    fn from(value: String) -> Self {
        Self::new(value.clone(), value)
    }
}

struct Proteins(Vec<Protein>);

impl<P: Into<Protein>> FromIterator<P> for Proteins {
    fn from_iter<T: IntoIterator<Item = P>>(iter: T) -> Self {
        Self(iter.into_iter().map(Into::into).collect())
    }
}

impl Proteins {
    fn from_fasta(fasta: impl Read) -> Result<Self> {
        Reader::new(fasta)
            .into_records()
            .collect::<Result<_, _>>()
            .map_err(Into::into)
    }

    fn from_txt(txt: impl Read) -> Result<Self> {
        BufReader::new(txt)
            .lines()
            .collect::<Result<_, _>>()
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

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

    #[test]
    fn from_fasta() {
        let expected = [
            ("P01013", "QIKDLLVSSSTDLDTTLVLVNAIYFKGMWKTAFNAEDTREMPFHVTKQESKPVQMMCMNNSFNVATLPAEKMKILELPFASGDLSMLVLLPDEVSDLERIEKTINFEKLTEWTNPNTMEKRRVKVYLPQMKIEEKYNLTSVLMALGMTDLFIPSANLTGISSAESLKISQAVHGAFMELSEDGIEMAGSTGVIEDIKHSPESEQFRADHPFLFLIKHNPTNTIVYFGRYWSP"),
            ("Z1072", "MKATKLVLGAVILGSTLLAGCSSNAKIDQLSSDVQTLNAKVDQLSNDVNAMRSDVQAAKDDAARANQRLDNMATKYRK"),
        ].map(|(id, sequence)| Protein::new(id, sequence));

        let proteins = Proteins::from_fasta(FASTA);
        assert!(proteins.is_ok());

        let proteins: Vec<_> = proteins.unwrap().0.into_iter().collect();

        assert_eq!(proteins, expected)
    }

    #[test]
    fn from_txt() {
        let expected = [
            ("AEJAA", "AEJAA"),
            ("AEJA", "AEJA"),
            ("AEJ", "AEJ"),
            ("AE", "AE"),
        ]
        .map(|(id, sequence)| Protein::new(id, sequence));

        let proteins = Proteins::from_txt(TXT);
        assert!(proteins.is_ok());

        let proteins: Vec<_> = proteins.unwrap().0.into_iter().collect();

        assert_eq!(proteins, expected)
    }
}
