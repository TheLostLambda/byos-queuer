// Standard Library Imports
use std::{
    ffi::OsStr,
    fs::{self, File},
    io::Seek,
    path::{self, Path, PathBuf},
    process::Command,
};

// External Crate Imports
use color_eyre::{Result, eyre::eyre};
use serde_json::Value;

// Local Crate Imports
use crate::{modifications::Modifications, proteins::Proteins, samples::Samples};

// Public API ==========================================================================================================

#[derive(Debug)]
pub struct Workflow {
    name: String,
    launch_command: Command,
}

impl Workflow {
    pub fn new(
        base_workflow: impl AsRef<Path> + Copy,
        sample_files: impl IntoIterator<Item = impl AsRef<Path>> + Copy,
        protein_file: impl AsRef<Path> + Copy,
        modifications_file: Option<impl AsRef<Path> + Copy>,
        output_directory: impl AsRef<Path> + Copy,
    ) -> Result<Self> {
        let name = Self::workflow_name(
            base_workflow,
            sample_files,
            protein_file,
            modifications_file,
        )?;

        let mut workflow_json: Value = serde_json::from_str(&fs::read_to_string(base_workflow)?)?;

        let samples = Self::load_samples(sample_files)?;
        let proteins = Self::load_proteins(protein_file)?;
        let modifications = Self::load_modifications(modifications_file)?;

        Self::update_json(&mut workflow_json, Self::SAMPLES_POINTER, samples)?;
        Self::update_json(&mut workflow_json, Self::PROTEINS_POINTER, proteins)?;
        Self::update_json(
            &mut workflow_json,
            Self::MODIFICATIONS_POINTER,
            modifications,
        )?;

        Self::write_wflw(output_directory, &name, &workflow_json)?;

        let launch_command = Self::build_command(output_directory, &name)?;

        Ok(Self {
            name,
            launch_command,
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn launch_command(&mut self) -> &mut Command {
        &mut self.launch_command
    }
}

// Private Helper Code =================================================================================================

impl Workflow {
    const SAMPLES_POINTER: &str = "/tabs/0/cells/0/properties/0/value/Samples";
    const PROTEINS_POINTER: &str = "/tabs/1/cells/0/properties/0/value/children";
    const MODIFICATIONS_POINTER: &str = "/tabs/2/cells/0/properties/13/value/modifications";

    const BYOS_EXE: &str = "PMi-Byos-Console.exe";

    fn workflow_name(
        base_workflow: impl AsRef<Path>,
        sample_files: impl IntoIterator<Item = impl AsRef<Path>>,
        protein_file: impl AsRef<Path>,
        modifications_file: Option<impl AsRef<Path>>,
    ) -> Result<String> {
        fn file_part(
            part: impl Fn(&Path) -> Option<&OsStr>,
            path: impl AsRef<Path>,
        ) -> Result<String> {
            let path = path.as_ref();
            part(path)
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
            "{base_workflow} ({sample_files}; {protein_file}{modifications_file})"
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

    fn update_json(json: &mut Value, pointer: &str, new_value: impl Into<Value>) -> Result<()> {
        let new_value = new_value.into();
        let target = json
            .pointer_mut(pointer)
            .ok_or_else(|| eyre!("failed to dereference JSON pointer: {pointer}"))?;

        *target = new_value;

        Ok(())
    }

    fn write_wflw(
        output_directory: impl AsRef<Path>,
        name: &str,
        workflow_json: &Value,
    ) -> Result<()> {
        let wflw_path = Self::wflw_path(output_directory, name);

        let mut wflw_file = File::create(&wflw_path)?;
        serde_json::to_writer_pretty(&mut wflw_file, workflow_json)?;

        Ok(())
    }

    fn build_command(output_directory: impl AsRef<Path> + Copy, name: &str) -> Result<Command> {
        let wflw_path = Self::wflw_path(output_directory, name);
        let output_path = path::absolute(output_directory.as_ref().join(name).join("Result"))?;

        let mut launch_command = Command::new(Self::BYOS_EXE);
        launch_command
            .arg("--mode=create-project")
            .arg("--input")
            .arg(wflw_path)
            .arg("--output")
            .arg(output_path);

        Ok(launch_command)
    }

    fn wflw_path(output_directory: impl AsRef<Path>, name: &str) -> PathBuf {
        // TODO: After https://github.com/rust-lang/rust/issues/127292 is stabilized, I should use one of those new
        // methods — `.with_extension()` was trying to replace something that wasn't actually an extension...
        output_directory.as_ref().join(format!("{name}.wflw"))
    }
}

// Unit Tests ==========================================================================================================

#[cfg(test)]
mod tests {
    use const_format::formatc;
    use serde_json::json;

    use super::*;

    const BASE_WORKFLOW: &str = "tests/data/PG Monomers.wflw";
    const SAMPLE_FILES: [&str; 2] = ["tests/data/WT.raw", "tests/data/6ldt.raw"];
    const PROTEIN_FASTA_FILE: &str = "tests/data/proteins.fasta";
    const PROTEIN_TXT_FILE: &str = "tests/data/proteins.txt";
    const MODIFICATIONS_FILE: &str = "tests/data/modifications.txt";
    const OUTPUT_DIRECTORY: &str = "tests/data/output";

    const WFLW_FILE: &str = formatc!("{OUTPUT_DIRECTORY}/{WORKFLOW_NAME}.wflw");
    const REFERENCE_WFLW_FILE: &str = formatc!("{OUTPUT_DIRECTORY}/Reference {WORKFLOW_NAME}.wflw");
    const RESULT_FILE: &str = formatc!("{OUTPUT_DIRECTORY}/{WORKFLOW_NAME}/Result");

    const WORKFLOW_NAME: &str = "PG Monomers (WT, 6ldt; proteins.fasta; modifications.txt)";

    fn load_wflw_json(path: impl AsRef<Path>) -> Value {
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn new() {
        // Test that .wflw file is created and matches reference output
        let _ = fs::remove_file(WFLW_FILE);
        assert!(!Path::new(WFLW_FILE).exists());

        let mut workflow = Workflow::new(
            BASE_WORKFLOW,
            SAMPLE_FILES,
            PROTEIN_FASTA_FILE,
            Some(MODIFICATIONS_FILE),
            OUTPUT_DIRECTORY,
        )
        .unwrap();

        assert!(Path::new(WFLW_FILE).exists());

        let workflow_json = load_wflw_json(WFLW_FILE);
        let reference_workflow_json = load_wflw_json(REFERENCE_WFLW_FILE);

        assert_eq!(workflow_json, reference_workflow_json);

        fs::remove_file(WFLW_FILE).unwrap();

        // Test that the returned `Workflow` has the correct `name` and `launch_command`
        let expected_result_file = path::absolute(RESULT_FILE).unwrap();
        let expected_args = vec![
            "--mode=create-project",
            "--input",
            dbg!(WFLW_FILE),
            "--output",
            expected_result_file.to_str().unwrap(),
        ];

        assert_eq!(workflow.name(), WORKFLOW_NAME);
        assert_eq!(workflow.launch_command().get_program(), Workflow::BYOS_EXE);
        assert_eq!(
            workflow.launch_command().get_args().collect::<Vec<_>>(),
            expected_args
        );
    }

    #[test]
    fn replace_samples() {
        let mut workflow_json = load_wflw_json(REFERENCE_WFLW_FILE);
        let samples = workflow_json.pointer(Workflow::SAMPLES_POINTER);
        assert!(samples.is_some());

        let samples_mut = samples.unwrap();
        assert!(samples_mut.is_array());

        let samples = samples_mut.as_array().unwrap();
        assert!(samples.iter().all(Value::is_object));
        assert_eq!(samples.len(), 2);

        let new_samples = json!([42, null, "wack"]);
        Workflow::update_json(
            &mut workflow_json,
            Workflow::SAMPLES_POINTER,
            new_samples.clone(),
        )
        .unwrap();

        let samples = workflow_json.pointer(Workflow::SAMPLES_POINTER).unwrap();
        assert_eq!(samples, &new_samples);
    }

    #[test]
    fn replace_proteins() {
        let mut workflow_json = load_wflw_json(REFERENCE_WFLW_FILE);
        let proteins = workflow_json.pointer(Workflow::PROTEINS_POINTER);
        assert!(proteins.is_some());

        let proteins_mut = proteins.unwrap();
        assert!(proteins_mut.is_array());

        let proteins = proteins_mut.as_array().unwrap();
        assert!(proteins.iter().all(Value::is_object));
        assert_eq!(proteins.len(), 2);

        let new_proteins = json!([42, null, "wack"]);
        Workflow::update_json(
            &mut workflow_json,
            Workflow::PROTEINS_POINTER,
            new_proteins.clone(),
        )
        .unwrap();

        let proteins = workflow_json.pointer(Workflow::PROTEINS_POINTER).unwrap();
        assert_eq!(proteins, &new_proteins);
    }

    #[test]
    fn replace_modifications() {
        let mut workflow_json = load_wflw_json(REFERENCE_WFLW_FILE);
        let modifications = workflow_json.pointer(Workflow::MODIFICATIONS_POINTER);
        assert!(modifications.is_some());

        let modifications_mut = modifications.unwrap();
        assert!(modifications_mut.is_string());

        let modifications = modifications_mut.as_str().unwrap();
        assert_eq!(modifications.len(), 122);

        let new_modifications = json!([42, null, "wack"]);
        Workflow::update_json(
            &mut workflow_json,
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
        let expected = "PG Monomers (WT, 6ldt; proteins.fasta)";

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
