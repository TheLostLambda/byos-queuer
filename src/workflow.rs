// Standard Library Imports
use std::{
    ffi::OsStr,
    fs::File,
    io::{BufReader, BufWriter, Seek, Write},
    path::{Path, PathBuf},
};

// External Crate Imports
use color_eyre::{Result, eyre::eyre};
use serde_json::Value;

use crate::{modifications::Modifications, proteins::Proteins, samples::Samples};

#[derive(Clone, Debug)]
pub struct Workflow {
    name: String,
    json: Value,
    output_directory: PathBuf,
}

impl Workflow {
    const SAMPLES_POINTER: &str = "/tabs/0/cells/0/properties/0/value/Samples";
    const PROTEINS_POINTER: &str = "/tabs/1/cells/0/properties/0/value/children";
    const MODIFICATIONS_POINTER: &str = "/tabs/2/cells/0/properties/13/value/modifications";

    pub fn new(
        base_workflow: impl AsRef<Path> + Copy,
        sample_files: impl IntoIterator<Item = impl AsRef<Path>> + Copy,
        protein_file: impl AsRef<Path> + Copy,
        modifications_file: Option<impl AsRef<Path> + Copy>,
        output_directory: impl AsRef<Path>,
    ) -> Result<Self> {
        let name = Self::workflow_name(
            base_workflow,
            sample_files,
            protein_file,
            modifications_file,
        )?;

        let workflow_reader = BufReader::new(File::open(base_workflow)?);
        let mut json: Value = serde_json::from_reader(workflow_reader)?;

        let samples = Self::load_samples(sample_files)?;
        let proteins = Self::load_proteins(protein_file)?;
        let modifications = Self::load_modifications(modifications_file)?;

        json = Self::replace_json(json, Self::SAMPLES_POINTER, samples.to_json())?;
        json = Self::replace_json(json, Self::PROTEINS_POINTER, proteins.to_json())?;
        json = Self::replace_json(json, Self::MODIFICATIONS_POINTER, modifications.to_json())?;

        let output_directory = output_directory.as_ref().to_owned();

        Ok(Self {
            name,
            json,
            output_directory,
        })
    }

    pub fn write_wflw(&self) -> Result<PathBuf> {
        let wflw_path = self.output_directory.join(&self.name);
        let mut wflw_writer = BufWriter::new(File::create(&wflw_path)?);

        serde_json::to_writer_pretty(&mut wflw_writer, &self.json)?;
        wflw_writer.flush()?;

        Ok(wflw_path)
    }

    fn workflow_name(
        base_workflow: impl AsRef<Path>,
        sample_files: impl IntoIterator<Item = impl AsRef<Path>>,
        protein_file: impl AsRef<Path>,
        modifications_file: Option<impl AsRef<Path>>,
    ) -> Result<String> {
        // FIXME: Should this really be a macro?
        fn file_part(
            part: impl Fn(&Path) -> Option<&OsStr>,
            path: impl AsRef<Path>,
        ) -> Result<String> {
            let path = path.as_ref();
            part(path)
                // PERF: This could be `OsStr::display` and the function could return `impl Display`, but that makes
                // using `.join()` later on a bit more complicated and requires this function to take a reference
                // instead of just `impl AsRef<Path>`...
                .map(|osstr| osstr.to_string_lossy().into_owned())
                .ok_or_else(|| eyre!("could not extract a file name from {}", path.display()))
        }

        let base_workflow = file_part(Path::file_stem, base_workflow)?;

        let sample_files = sample_files
            .into_iter()
            .map(|path| file_part(Path::file_stem, path))
            .collect::<Result<Vec<_>>>()?
            .join(", ");

        let protein_file = file_part(Path::file_name, protein_file)?;

        let modifications_file = if let Some(modifications_file) = modifications_file {
            let modifications_file = file_part(Path::file_name, modifications_file)?;
            format!("; {modifications_file}")
        } else {
            String::new()
        };

        Ok(format!(
            "{base_workflow} ({sample_files}; {protein_file}{modifications_file}).wflw"
        ))
    }

    fn load_samples(
        sample_files: impl IntoIterator<Item = impl AsRef<Path>> + Copy,
    ) -> Result<Samples> {
        // NOTE: Though this program never actually needs to open any of the sample files, it's worth making sure that
        // the paths exist now so that any errors are reported early instead of when a Byos search fails in the future
        let missing_file = sample_files
            .into_iter()
            .find(|path| !path.as_ref().exists());
        if let Some(missing_file) = missing_file {
            let missing_file = missing_file.as_ref().display();
            return Err(eyre!("failed to find `{missing_file}`"));
        }

        // PERF: There is a lot of unnecessary cloning going on here, but I don't think that matters much at the moment
        Ok(sample_files
            .into_iter()
            .map(|path| path.as_ref().to_path_buf())
            .collect())
    }

    fn load_proteins(protein_file: impl AsRef<Path>) -> Result<Proteins> {
        let mut file = File::open(protein_file)?;
        Proteins::from_fasta(file.try_clone()?).or_else(|_| {
            file.rewind()?;
            Proteins::from_txt(file)
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

    fn replace_json(mut json: Value, pointer: impl AsRef<str>, new_value: Value) -> Result<Value> {
        let pointer = pointer.as_ref();
        let target = json
            .pointer_mut(pointer)
            .ok_or_else(|| eyre!("failed to dereference JSON pointer: {pointer}"))?;

        *target = new_value;

        Ok(json)
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, sync::LazyLock};

    use const_format::formatc;
    use serde_json::json;

    use super::*;

    const BASE_WORKFLOW: &str = "tests/data/PG Monomers.wflw";
    const SAMPLE_FILES: [&str; 2] = ["tests/data/WT.raw", "tests/data/6ldt.raw"];
    const PROTEIN_FASTA_FILE: &str = "tests/data/proteins.fasta";
    const PROTEIN_TXT_FILE: &str = "tests/data/proteins.txt";
    const MODIFICATIONS_FILE: &str = "tests/data/modifications.txt";

    const OUTPUT_DIRECTORY: &str = "tests/data/output/";
    const WORKFLOW_NAME: &str = "PG Monomers (WT, 6ldt; proteins.fasta; modifications.txt).wflw";
    const REFERENCE_WORKFLOW: &str = formatc!("{OUTPUT_DIRECTORY}/Reference {WORKFLOW_NAME}");

    static WORKFLOW: LazyLock<Workflow> = LazyLock::new(|| {
        Workflow::new(
            BASE_WORKFLOW,
            SAMPLE_FILES,
            PROTEIN_FASTA_FILE,
            Some(MODIFICATIONS_FILE),
            OUTPUT_DIRECTORY,
        )
        .unwrap()
    });

    #[test]
    fn new() {
        let expected_samples = &json!([
            {
                "CustomColumnsData": [],
                "Digestion": "",
                "InSilicoList": "",
                "Metadata": {
                    "remote_name": "",
                    "remote_path": "",
                    "server": "",
                    "type": 0,
                    "uuid": ""
                },
                "Ms2File": [],
                "Ms2Search": [],
                "MsFile": "tests/data/WT.raw",
                "SampleName": "WT",
                "SampleType": "Reference",
                "SamplesId": 1,
                "TraceFiles": []
            },
            {
                "CustomColumnsData": [],
                "Digestion": "",
                "InSilicoList": "",
                "Metadata": {
                    "remote_name": "",
                    "remote_path": "",
                    "server": "",
                    "type": 0,
                    "uuid": ""
                },
                "Ms2File": [],
                "Ms2Search": [],
                "MsFile": "tests/data/6ldt.raw",
                "SampleName": "6ldt",
                "SampleType": "Reference",
                "SamplesId": 2,
                "TraceFiles": []
            },
        ]);
        let expected_proteins = &json!([
           {
               "children": [],
               "data": [
                   "P01013",
                   "",
                   "QIKDLLVSSSTDLDTTLVLVNAIYFKGMWKTAFNAEDTREMPFHVTKQESKPVQMMCMNNSFNVATLPAEKMKILELPFASGDLSMLVLLPDEVSDLERIEKTINFEKLTEWTNPNTMEKRRVKVYLPQMKIEEKYNLTSVLMALGMTDLFIPSANLTGISSAESLKISQAVHGAFMELSEDGIEMAGSTGVIEDIKHSPESEQFRADHPFLFLIKHNPTNTIVYFGRYWSP",
                   null,
                   1
               ]
           },
           {
               "children": [],
               "data": [
                   "Z1072",
                   "",
                   "MKATKLVLGAVILGSTLLAGCSSNAKIDQLSSDVQTLNAKVDQLSNDVNAMRSDVQAAKDDAARANQRLDNMATKYRK",
                   null,
                   2
               ]
           },
        ]);
        let expected_modifications = &json!(
            "% Custom modification text below\nHexNAc(1)MurNAc_alditol(1) @ NTerm | common1\nHexN(1) MurNAc_alditol(1) @ NTerm | common1\n"
        );

        let samples = WORKFLOW.json.pointer(Workflow::SAMPLES_POINTER).unwrap();
        let proteins = WORKFLOW.json.pointer(Workflow::PROTEINS_POINTER).unwrap();
        let modifications = WORKFLOW
            .json
            .pointer(Workflow::MODIFICATIONS_POINTER)
            .unwrap();

        assert_eq!(samples, expected_samples);
        assert_eq!(proteins, expected_proteins);
        assert_eq!(modifications, expected_modifications);

        let expected_output_directory = PathBuf::from(OUTPUT_DIRECTORY);

        assert_eq!(WORKFLOW.name, WORKFLOW_NAME);
        assert_eq!(WORKFLOW.output_directory, expected_output_directory);
    }

    #[test]
    fn write_wflw() {
        let expected_path = PathBuf::from(
            "tests/data/output/PG Monomers (WT, 6ldt; proteins.fasta; modifications.txt).wflw",
        );
        let _ = fs::remove_file(&expected_path);
        assert!(!expected_path.exists());

        let workflow_name = WORKFLOW.write_wflw();
        assert!(workflow_name.is_ok());
        assert_eq!(workflow_name.unwrap(), expected_path);

        assert!(expected_path.exists());
        assert_eq!(
            fs::read_to_string(&expected_path).unwrap(),
            fs::read_to_string(REFERENCE_WORKFLOW).unwrap()
        );

        fs::remove_file(expected_path).unwrap();
    }

    #[test]
    fn replace_samples() {
        let workflow_json = WORKFLOW.json.clone();
        let samples = workflow_json.pointer(Workflow::SAMPLES_POINTER);
        assert!(samples.is_some());

        let samples_mut = samples.unwrap();
        assert!(samples_mut.is_array());

        let samples = samples_mut.as_array().unwrap();
        assert!(samples.iter().all(Value::is_object));
        assert_eq!(samples.len(), 2);

        let new_samples = json!([42, null, "wack"]);
        let workflow_json = Workflow::replace_json(
            workflow_json,
            Workflow::SAMPLES_POINTER,
            new_samples.clone(),
        )
        .unwrap();

        let samples = workflow_json.pointer(Workflow::SAMPLES_POINTER).unwrap();
        assert_eq!(samples, &new_samples);
    }

    #[test]
    fn replace_proteins() {
        let workflow_json = WORKFLOW.json.clone();
        let proteins = workflow_json.pointer(Workflow::PROTEINS_POINTER);
        assert!(proteins.is_some());

        let proteins_mut = proteins.unwrap();
        assert!(proteins_mut.is_array());

        let proteins = proteins_mut.as_array().unwrap();
        assert!(proteins.iter().all(Value::is_object));
        assert_eq!(proteins.len(), 2);

        let new_proteins = json!([42, null, "wack"]);
        let workflow_json = Workflow::replace_json(
            workflow_json,
            Workflow::PROTEINS_POINTER,
            new_proteins.clone(),
        )
        .unwrap();

        let proteins = workflow_json.pointer(Workflow::PROTEINS_POINTER).unwrap();
        assert_eq!(proteins, &new_proteins);
    }

    #[test]
    fn replace_modifications() {
        let workflow_json = WORKFLOW.json.clone();
        let modifications = workflow_json.pointer(Workflow::MODIFICATIONS_POINTER);
        assert!(modifications.is_some());

        let modifications_mut = modifications.unwrap();
        assert!(modifications_mut.is_string());

        let modifications = modifications_mut.as_str().unwrap();
        assert_eq!(modifications.len(), 122);

        let new_modifications = json!([42, null, "wack"]);
        let workflow_json = Workflow::replace_json(
            workflow_json,
            Workflow::MODIFICATIONS_POINTER,
            new_modifications.clone(),
        )
        .unwrap();

        let modifications = workflow_json
            .pointer(Workflow::MODIFICATIONS_POINTER)
            .unwrap();
        assert_eq!(modifications, &new_modifications);
    }

    #[test]
    fn workflow_name_with_modifications() {
        let workflow_name = Workflow::workflow_name(
            BASE_WORKFLOW,
            SAMPLE_FILES,
            PROTEIN_FASTA_FILE,
            Some(MODIFICATIONS_FILE),
        );
        assert!(workflow_name.is_ok());

        assert_eq!(workflow_name.unwrap(), WORKFLOW_NAME);
    }

    #[test]
    fn workflow_name_without_modifications() {
        let expected = "PG Monomers (WT, 6ldt; proteins.fasta).wflw";

        let workflow_name = Workflow::workflow_name(
            BASE_WORKFLOW,
            SAMPLE_FILES,
            PROTEIN_FASTA_FILE,
            None::<&str>,
        );
        assert!(workflow_name.is_ok());

        assert_eq!(workflow_name.unwrap(), expected);
    }

    #[test]
    fn load_samples() {
        let expected = SAMPLE_FILES.into_iter().collect();

        let samples = Workflow::load_samples(SAMPLE_FILES);
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
