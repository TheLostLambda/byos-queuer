// Standard Library Imports
use std::path::PathBuf;

// External Crate Imports
use serde_json::{Value, json};

// Public API ==========================================================================================================

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Samples(Vec<Sample>);

impl From<&Samples> for Value {
    fn from(value: &Samples) -> Self {
        let samples = value
            .0
            .iter()
            .enumerate()
            .map(|(index, sample)| {
                let sample_name = sample.file_stem().unwrap_or_default().to_string_lossy();
                json!({
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
                    "MsFile": sample,
                    "SampleName": sample_name,
                    "SampleType": "Reference",
                    "SamplesId": index + 1,
                    "TraceFiles": []
                })
            })
            .collect();

        Self::Array(samples)
    }
}

impl From<Samples> for Value {
    fn from(value: Samples) -> Self {
        Self::from(&value)
    }
}

impl<S: Into<Sample>> FromIterator<S> for Samples {
    fn from_iter<T: IntoIterator<Item = S>>(iter: T) -> Self {
        Self(iter.into_iter().map(Into::into).collect())
    }
}

// Private Helper Code =================================================================================================

type Sample = PathBuf;

// Unit Tests ==========================================================================================================

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use serde_json::json;

    use super::*;

    static SAMPLES: LazyLock<Samples> = LazyLock::new(|| {
        [
            "C:/Users/MBB-Byonic/Desktop/Data to deconvolute March 2025/Fuso_3.raw",
            "C:/Users/MBB-Byonic/Desktop/Data to deconvolute March 2025/Fuso_2.raw",
            "C:/Users/MBB-Byonic/Desktop/Data to deconvolute March 2025/Fuso_1.raw",
        ]
        .into_iter()
        .collect()
    });

    #[test]
    fn to_json() {
        let expected = json!([
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
                "MsFile": "C:/Users/MBB-Byonic/Desktop/Data to deconvolute March 2025/Fuso_3.raw",
                "SampleName": "Fuso_3",
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
                "MsFile": "C:/Users/MBB-Byonic/Desktop/Data to deconvolute March 2025/Fuso_2.raw",
                "SampleName": "Fuso_2",
                "SampleType": "Reference",
                "SamplesId": 2,
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
                "MsFile": "C:/Users/MBB-Byonic/Desktop/Data to deconvolute March 2025/Fuso_1.raw",
                "SampleName": "Fuso_1",
                "SampleType": "Reference",
                "SamplesId": 3,
                "TraceFiles": []
            }
        ]);

        let json = Value::from(&*SAMPLES);

        assert_eq!(json, expected);
    }
}
