// Standard Library Imports
use std::{
    ffi::OsStr,
    fs::{self, File},
    io::Seek,
    path::{self, Path, PathBuf},
};

// External Crate Imports
use color_eyre::{
    Result,
    eyre::{Context, eyre},
};
use duct::{Handle, cmd};
use serde_json::Value;

// Local Crate Imports
use crate::{modifications::Modifications, proteins::Proteins, samples::Samples};

// Public API ==========================================================================================================

pub struct Workflow {
    name: String,
    launch_command: Box<dyn Fn() -> Result<Handle> + Send + Sync>,
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

    pub fn start(&self) -> Result<Handle> {
        (self.launch_command)()
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

    fn build_command(
        output_directory: impl AsRef<Path> + Copy,
        name: &str,
    ) -> Result<Box<dyn Fn() -> Result<Handle> + Send + Sync>> {
        let wflw_path = Self::wflw_path(output_directory, name);
        let result_path = path::absolute(output_directory.as_ref().join(name))?;
        let result_file = result_path.join("Result");
        let log_file = result_path.join("log.txt");

        let name = name.to_owned();
        let launch_command = move || {
            if !result_path.exists() {
                fs::create_dir(&result_path)?;
            }

            cmd!(
                Self::BYOS_EXE,
                "--mode=create-project",
                "--input",
                &wflw_path,
                "--output",
                &result_file
            )
            .stderr_to_stdout()
            .stdout_path(&log_file)
            .start()
            .wrap_err_with(|| format!("failed to run workflow {name}"))
        };

        Ok(Box::new(launch_command))
    }

    fn wflw_path(output_directory: impl AsRef<Path>, name: &str) -> PathBuf {
        // TODO: After https://github.com/rust-lang/rust/issues/127292 is stabilized, I should use one of those new
        // methods — `.with_extension()` was trying to replace something that wasn't actually an extension...
        output_directory.as_ref().join(format!("{name}.wflw"))
    }
}

// Unit Tests ==========================================================================================================

#[cfg(test)]
pub(crate) mod tests {
    use std::env;

    use const_format::formatc;
    use serde_json::json;
    use tempfile::tempdir;

    use super::*;

    pub const BASE_WORKFLOW: &str = "tests/data/PG Monomers.wflw";
    pub const SAMPLE_FILES: [&str; 2] = ["tests/data/WT.raw", "tests/data/6ldt.raw"];
    pub const PROTEIN_FASTA_FILE: &str = "tests/data/proteins.fasta";
    pub const MODIFICATIONS_FILE: &str = "tests/data/modifications.txt";

    pub const WORKFLOW_NAME: &str = "PG Monomers (WT, 6ldt; proteins.fasta; modifications.txt)";

    pub fn result_directory_in(temporary_directory: impl AsRef<Path>) -> PathBuf {
        temporary_directory.as_ref().join(WORKFLOW_NAME)
    }

    pub fn wflw_file_in(temporary_directory: impl AsRef<Path>) -> PathBuf {
        // TODO: Keep an eye on https://github.com/rust-lang/rust/issues/127292 for a better way to do this...
        let mut result_directory = result_directory_in(temporary_directory).into_os_string();
        result_directory.push(".wflw");
        PathBuf::from(result_directory)
    }

    pub fn result_file_in(temporary_directory: impl AsRef<Path>) -> PathBuf {
        result_directory_in(temporary_directory).join("Result")
    }

    // SAFETY: It's unsafe for multiple threads to call `env::set_var()`, but given I'm using cargo-nextest which
    // launches each test as its own process, it should be alright... Honestly, even if it does result in unsafe
    // behaviour, these are just tests, so I don't reckon it can do too much damage...
    pub unsafe fn with_test_path<T>(path: impl AsRef<Path>, test_code: impl FnOnce() -> T) -> T {
        // TODO: Use `cfg` to change this to `;` on Windows!
        const PATH_SEPARATOR: &str = ":";

        let old_path = env::var("PATH").unwrap_or_default();
        let new_path = path::absolute(path).unwrap();
        let joined_path = format!(
            "{new_path}{PATH_SEPARATOR}{old_path}",
            new_path = new_path.display()
        );

        unsafe {
            env::set_var("PATH", joined_path);
        }

        let result = test_code();

        unsafe {
            env::set_var("PATH", old_path);
        }

        result
    }

    const PROTEIN_TXT_FILE: &str = "tests/data/proteins.txt";

    const REFERENCE_WFLW_FILE: &str = formatc!("tests/data/output/Reference {WORKFLOW_NAME}.wflw");

    fn log_file_in(temporary_directory: impl AsRef<Path>) -> PathBuf {
        result_directory_in(temporary_directory).join("log.txt")
    }

    // TODO: To get these tests working on Windows, I think I'll need to create `scripts/windows` and `scripts/unix`
    // directories and use conditional compilation to set `TEST_SCRIPTS` accordingly!
    const TEST_PATH: &str = "tests/scripts/workflow";

    fn load_wflw_json(path: impl AsRef<Path>) -> Value {
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn new_then_run() {
        let temporary_directory = tempdir().unwrap();
        let wflw_file = wflw_file_in(&temporary_directory);
        let result_directory = result_directory_in(&temporary_directory);
        let log_file = log_file_in(&temporary_directory);
        let result_file = result_file_in(&temporary_directory);

        // Test that .wflw file is created and matches reference output
        assert!(!wflw_file.exists());

        let workflow = Workflow::new(
            BASE_WORKFLOW,
            SAMPLE_FILES,
            PROTEIN_FASTA_FILE,
            Some(MODIFICATIONS_FILE),
            &temporary_directory,
        )
        .unwrap();

        assert!(&wflw_file.exists());

        let workflow_json = load_wflw_json(&wflw_file);
        let reference_workflow_json = load_wflw_json(REFERENCE_WFLW_FILE);

        assert_eq!(workflow_json, reference_workflow_json);

        // Test that the returned `Workflow` has the correct `name` and `launch_command`
        assert_eq!(workflow.name(), WORKFLOW_NAME);

        assert!(!result_directory.exists());

        let handle = unsafe { with_test_path(TEST_PATH, || workflow.start()) }.unwrap();
        let output = handle.wait();
        assert!(output.is_ok());

        let merged_output = fs::read_to_string(log_file).unwrap();
        let mut lines = merged_output.lines();

        let executable_path = Path::new(lines.next().unwrap());
        assert!(executable_path.is_absolute());
        assert!(executable_path.ends_with(Workflow::BYOS_EXE));
        assert_eq!(lines.next(), Some("--mode=create-project"));
        assert_eq!(lines.next(), Some("--input"));
        assert_eq!(lines.next(), Some(wflw_file.to_str().unwrap()));
        assert_eq!(lines.next(), Some("--output"));
        let result_file_path = Path::new(lines.next().unwrap());
        assert!(result_file_path.is_absolute());
        assert!(result_file_path.ends_with(result_file));

        // Make sure that `Workflow`s can be re-run without panicking
        let handle = unsafe { with_test_path(TEST_PATH, || workflow.start()) }.unwrap();
        let output = handle.wait();
        assert!(output.is_ok());
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
