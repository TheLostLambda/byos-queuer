// Standard Library Imports
use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

// External Crate Imports
use color_eyre::Result;
use serde_json::Value;

use crate::samples::Samples;

#[derive(Clone, Debug)]
struct Workflow {
    workflow_name: String,
    workflow_json: Value,
    output_directory: PathBuf,
}

impl Workflow {
    const SAMPLES_POINTER: &str = "/tabs/0/cells/0/properties/0/value/Samples";
    const PROTEINS_POINTER: &str = "/tabs/1/cells/0/properties/0/value/children";
    const MODIFICATIONS_POINTER: &str = "/tabs/2/cells/0/properties/13/value/modifications";

    pub fn new(
        base_workflow: impl AsRef<Path>,
        sample_files: Vec<impl AsRef<Path>>,
        protein_file: impl AsRef<Path>,
        modifications_file: Option<impl AsRef<Path>>,
        output_directory: impl AsRef<Path>,
    ) -> Result<Self> {
        let file = File::open(base_workflow)?;
        let reader = BufReader::new(file);
        let mut workflow_json: Value = serde_json::from_reader(reader)?;

        let output_directory = output_directory.as_ref().to_owned();

        Ok(Workflow {
            workflow_name: String::new(),
            workflow_json,
            output_directory,
        })
    }
    pub fn to_wflw(&self) -> String {
        todo!()
    }

    fn load_samples(sample_files: Vec<impl AsRef<Path>>) -> Samples {
        // PERF: There is a lot of unnecessary cloning going on here, but I don't think that matters much at the moment
        sample_files.iter().map(AsRef::as_ref).to_owned().collect()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use serde_json::json;

    use super::*;

    static WORKFLOW: LazyLock<Workflow> = LazyLock::new(|| {
        Workflow::new(
            "./tests/data/PG Monomers (MS2).wflw",
            vec!["./"],
            PathBuf::new(),
            Some(PathBuf::new()),
            PathBuf::new(),
        )
        .unwrap()
    });

    #[test]
    fn samples_mut() {
        let mut workflow_json = WORKFLOW.workflow_json.clone();
        let samples_mut = workflow_json.pointer_mut(Workflow::SAMPLES_POINTER);
        assert!(samples_mut.is_some());

        let samples_mut = samples_mut.unwrap();
        assert!(samples_mut.is_array());

        let samples = samples_mut.as_array().unwrap();
        assert!(samples.iter().all(|obj| obj.is_object()));
        assert_eq!(samples.len(), 3);

        let new_samples = json!([42, null, "wack"]);
        *samples_mut = new_samples.clone();

        let samples = workflow_json.pointer(Workflow::SAMPLES_POINTER).unwrap();
        assert_eq!(samples, &new_samples);
    }

    #[test]
    fn proteins_mut() {
        let mut workflow_json = WORKFLOW.workflow_json.clone();
        let proteins_mut = workflow_json.pointer_mut(Workflow::PROTEINS_POINTER);
        assert!(proteins_mut.is_some());

        let proteins_mut = proteins_mut.unwrap();
        assert!(proteins_mut.is_array());

        let proteins = proteins_mut.as_array().unwrap();
        assert!(proteins.iter().all(|obj| obj.is_object()));
        assert_eq!(proteins.len(), 42);

        let new_proteins = json!([42, null, "wack"]);
        *proteins_mut = new_proteins.clone();

        let proteins = workflow_json.pointer(Workflow::PROTEINS_POINTER).unwrap();
        assert_eq!(proteins, &new_proteins);
    }

    #[test]
    fn modifications_mut() {
        let mut workflow_json = WORKFLOW.workflow_json.clone();
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
}
