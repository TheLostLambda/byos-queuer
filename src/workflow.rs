// Standard Library Imports
use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

// External Crate Imports
use color_eyre::{eyre::OptionExt, Result};
use serde_json::Value;

use crate::samples::Samples;

#[derive(Clone, Debug)]
struct WorkflowParameters {
    base_workflow: PathBuf,
    sample_files: Vec<PathBuf>,
    protein_file: PathBuf,
    modifications_file: Option<PathBuf>,
    output_directory: PathBuf,
}

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
        let base_workflow: Value = serde_json::from_reader(reader)?;
        todo!()
    }
    pub fn to_wflw(&self) -> String {
        let file = File::open(&self.base_workflow).unwrap();
        let reader = BufReader::new(file);
        let base_workflow: Value = serde_json::from_reader(reader).unwrap();
        todo!()
    }

    fn load_samples(&self) -> Samples {
        // PERF: There is a lot of unnecessary cloning going on here, but I don't think that matters much at the moment
        self.sample_files.iter().cloned().collect()
    }

    fn samples_mut(&mut self) -> Result<&mut Value> {
        self.base_workflow
            .pointer_mut(Self::SAMPLES_POINTER)
            .ok_or_eyre("the base workflow didn't contain the samples key at the right path")
    }

    fn proteins_mut(&mut self) -> Result<&mut Value> {
        self.base_workflow
            .pointer_mut(Self::PROTEINS_POINTER)
            .ok_or_eyre("the base workflow didn't contain the proteins key at the right path")
    }

    fn modifications_mut(&mut self) -> Result<&mut Value> {
        self.base_workflow
            .pointer_mut(Self::MODIFICATIONS_POINTER)
            .ok_or_eyre("the base workflow didn't contain the modifications key at the right path")
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, sync::LazyLock};

    use serde_json::json;

    use super::*;

    static WORKFLOW: LazyLock<WorkflowParameters> = LazyLock::new(|| {
        let file = File::open("./tests/data/PG Monomers (MS2).wflw").unwrap();
        let reader = BufReader::new(file);
        let base_workflow = serde_json::from_reader(reader).unwrap();

        WorkflowParameters {
            base_workflow,
            sample_files: vec![],
            protein_file: PathBuf::new(),
            modifications_file: None,
            output_directory: PathBuf::new(),
        }
    });

    #[test]
    fn samples_mut() {
        let mut workflow = WORKFLOW.clone();
        let samples_mut = workflow.samples_mut();
        assert!(samples_mut.is_ok());

        let samples_mut = samples_mut.unwrap();
        assert!(samples_mut.is_array());

        let samples = samples_mut.as_array().unwrap();
        assert!(samples.iter().all(|obj| obj.is_object()));
        assert_eq!(samples.len(), 3);

        let new_samples = json!([42, null, "wack"]);
        *samples_mut = new_samples.clone();

        let samples = workflow.samples_mut().unwrap();
        assert_eq!(samples, &new_samples);
    }

    #[test]
    fn proteins_mut() {
        let mut workflow = WORKFLOW.clone();
        let proteins_mut = workflow.proteins_mut();
        assert!(proteins_mut.is_ok());

        let proteins_mut = proteins_mut.unwrap();
        assert!(proteins_mut.is_array());

        let proteins = proteins_mut.as_array().unwrap();
        assert!(proteins.iter().all(|obj| obj.is_object()));
        assert_eq!(proteins.len(), 42);

        let new_proteins = json!([42, null, "wack"]);
        *proteins_mut = new_proteins.clone();

        let proteins = workflow.proteins_mut().unwrap();
        assert_eq!(proteins, &new_proteins);
    }

    #[test]
    fn modifications_mut() {
        let mut workflow = WORKFLOW.clone();
        let modifications_mut = workflow.modifications_mut();
        assert!(modifications_mut.is_ok());

        let modifications_mut = modifications_mut.unwrap();
        assert!(modifications_mut.is_string());

        let modifications = modifications_mut.as_str().unwrap();
        assert_eq!(modifications.len(), 306);

        let new_modifications = json!([42, null, "wack"]);
        *modifications_mut = new_modifications.clone();

        let modifications = workflow.modifications_mut().unwrap();
        assert_eq!(modifications, &new_modifications);
    }
}
