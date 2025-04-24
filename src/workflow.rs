// Standard Library Imports
use std::{
    fs::File,
    io::{BufReader, Seek},
    path::{Path, PathBuf},
};

// External Crate Imports
use color_eyre::{eyre::eyre, Result};
use serde_json::Value;

use crate::{modifications::Modifications, proteins::Proteins, samples::Samples};

#[derive(Clone, Debug)]
struct Workflow {
    name: String,
    json: Value,
    output_directory: PathBuf,
}

impl Workflow {
    const SAMPLES_POINTER: &str = "/tabs/0/cells/0/properties/0/value/Samples";
    const PROTEINS_POINTER: &str = "/tabs/1/cells/0/properties/0/value/children";
    const MODIFICATIONS_POINTER: &str = "/tabs/2/cells/0/properties/13/value/modifications";

    pub fn new(
        base_workflow: impl AsRef<Path>,
        sample_files: &[impl AsRef<Path>],
        protein_file: impl AsRef<Path>,
        modifications_file: Option<impl AsRef<Path>>,
        output_directory: impl AsRef<Path>,
    ) -> Result<Self> {
        let workflow_reader = BufReader::new(File::open(base_workflow)?);
        let mut workflow_json: Value = serde_json::from_reader(workflow_reader)?;

        let output_directory = output_directory.as_ref().to_owned();

        Ok(Self {
            name: String::new(),
            json: workflow_json,
            output_directory,
        })
    }
    pub fn to_wflw(&self) -> String {
        todo!()
    }

    fn load_samples(sample_files: &[impl AsRef<Path>]) -> Result<Samples> {
        // NOTE: Though this program never actually needs to open any of the sample files, it's worth making sure that
        // the paths exist now so that any errors are reported early instead of when a Byos search fails in the future
        let missing_file = sample_files.iter().find(|&path| !path.as_ref().exists());
        if let Some(missing_file) = missing_file {
            let missing_file = missing_file.as_ref().display();
            return Err(eyre!("failed to find `{missing_file}`"));
        }

        // PERF: There is a lot of unnecessary cloning going on here, but I don't think that matters much at the moment
        Ok(sample_files.iter().map(AsRef::as_ref).clone().collect())
    }

    fn load_proteins(protein_file: impl AsRef<Path>) -> Result<Proteins> {
        let mut file = File::open(protein_file)?;
        Proteins::from_fasta(file.try_clone()?).or_else(|_| {
            file.rewind()?;
            Proteins::from_txt(dbg!(file))
        })
    }

    fn load_modifications(modifications_file: Option<impl AsRef<Path>>) -> Result<Modifications> {
        if let Some(modifications_file) = modifications_file {
            let file = File::open(modifications_file)?;
            Modifications::from_txt(file)
        } else {
            Ok(Modifications::default())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use serde_json::json;

    use super::*;

    const SAMPLE_FILES: [&str; 2] = ["tests/data/WT.raw", "tests/data/6ldt.raw"];

    const PROTEIN_FASTA_FILE: &str = "tests/data/proteins.fasta";
    const PROTEIN_TXT_FILE: &str = "tests/data/proteins.txt";
    const MODIFICATIONS_FILE: &str = "tests/data/modifications.txt";

    static WORKFLOW: LazyLock<Workflow> = LazyLock::new(|| {
        Workflow::new(
            "tests/data/PG Monomers (MS2).wflw",
            &SAMPLE_FILES,
            PathBuf::new(),
            Some(PathBuf::new()),
            PathBuf::new(),
        )
        .unwrap()
    });

    #[test]
    fn samples_mut() {
        let mut workflow_json = WORKFLOW.json.clone();
        let samples_mut = workflow_json.pointer_mut(Workflow::SAMPLES_POINTER);
        assert!(samples_mut.is_some());

        let samples_mut = samples_mut.unwrap();
        assert!(samples_mut.is_array());

        let samples = samples_mut.as_array().unwrap();
        assert!(samples.iter().all(Value::is_object));
        assert_eq!(samples.len(), 3);

        let new_samples = json!([42, null, "wack"]);
        *samples_mut = new_samples.clone();

        let samples = workflow_json.pointer(Workflow::SAMPLES_POINTER).unwrap();
        assert_eq!(samples, &new_samples);
    }

    #[test]
    fn proteins_mut() {
        let mut workflow_json = WORKFLOW.json.clone();
        let proteins_mut = workflow_json.pointer_mut(Workflow::PROTEINS_POINTER);
        assert!(proteins_mut.is_some());

        let proteins_mut = proteins_mut.unwrap();
        assert!(proteins_mut.is_array());

        let proteins = proteins_mut.as_array().unwrap();
        assert!(proteins.iter().all(Value::is_object));
        assert_eq!(proteins.len(), 42);

        let new_proteins = json!([42, null, "wack"]);
        *proteins_mut = new_proteins.clone();

        let proteins = workflow_json.pointer(Workflow::PROTEINS_POINTER).unwrap();
        assert_eq!(proteins, &new_proteins);
    }

    #[test]
    fn modifications_mut() {
        let mut workflow_json = WORKFLOW.json.clone();
        let modifications_mut = workflow_json.pointer_mut(Workflow::MODIFICATIONS_POINTER);
        assert!(modifications_mut.is_some());

        let modifications_mut = modifications_mut.unwrap();
        assert!(modifications_mut.is_string());

        let modifications = modifications_mut.as_str().unwrap();
        assert_eq!(modifications.len(), 306);

        let new_modifications = json!([42, null, "wack"]);
        *modifications_mut = new_modifications.clone();

        let modifications = workflow_json
            .pointer(Workflow::MODIFICATIONS_POINTER)
            .unwrap();
        assert_eq!(modifications, &new_modifications);
    }

    #[test]
    fn load_samples() {
        let expected = SAMPLE_FILES.into_iter().collect();

        let samples = Workflow::load_samples(&SAMPLE_FILES);
        assert!(samples.is_ok());

        assert_eq!(samples.unwrap(), expected);
    }

    #[test]
    fn load_proteins_fasta() {
        let expected = [
            ("P01013", "QIKDLLVSSSTDLDTTLVLVNAIYFKGMWKTAFNAEDTREMPFHVTKQESKPVQMMCMNNSFNVATLPAEKMKILELPFASGDLSMLVLLPDEVSDLERIEKTINFEKLTEWTNPNTMEKRRVKVYLPQMKIEEKYNLTSVLMALGMTDLFIPSANLTGISSAESLKISQAVHGAFMELSEDGIEMAGSTGVIEDIKHSPESEQFRADHPFLFLIKHNPTNTIVYFGRYWSP"),
            ("Z1072", "MKATKLVLGAVILGSTLLAGCSSNAKIDQLSSDVQTLNAKVDQLSNDVNAMRSDVQAAKDDAARANQRLDNMATKYRK"),
        ].into_iter().collect();

        let proteins = Workflow::load_proteins(PROTEIN_FASTA_FILE);
        assert!(proteins.is_ok());

        assert_eq!(proteins.unwrap(), expected);
    }

    #[test]
    fn load_proteins_txt() {
        let expected = [
            ("AEJAA", "AEJAA"),
            ("AEJA", "AEJA"),
            ("AEJ", "AEJ"),
            ("AE", "AE"),
        ]
        .into_iter()
        .collect();

        let proteins = Workflow::load_proteins(PROTEIN_TXT_FILE);
        assert!(proteins.is_ok());

        assert_eq!(proteins.unwrap(), expected);
    }

    #[test]
    fn load_modifications() {
        let expected = Modifications("% Custom modification text below\nHexNAc(1)MurNAc_alditol(1) @ NTerm | common1\nHexN(1) MurNAc_alditol(1) @ NTerm | common1\n".to_owned());

        let modifications = Workflow::load_modifications(Some(MODIFICATIONS_FILE));
        assert!(modifications.is_ok());

        assert_eq!(modifications.unwrap(), expected);

        let empty_modifications = Workflow::load_modifications(None::<&str>);
        assert_eq!(empty_modifications.unwrap(), Modifications::default());
    }
}
