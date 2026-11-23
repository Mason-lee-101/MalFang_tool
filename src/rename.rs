use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use clap::ValueEnum;
use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum HashAlgorithm {
    Md5,
    #[value(alias = "sha-1")]
    Sha1,
    #[value(alias = "sha-256")]
    Sha256,
    #[value(alias = "sha-512")]
    Sha512,
    Blake3,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RenameOperation {
    Defang(HashAlgorithm),
    Refang(HashAlgorithm),
}

impl RenameOperation {
    pub(crate) fn label_past_tense(&self) -> &'static str {
        match self {
            Self::Defang(_) => "Defanged",
            Self::Refang(_) => "Refanged",
        }
    }
}

impl HashAlgorithm {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Md5 => "MD5",
            Self::Sha1 => "SHA-1",
            Self::Sha256 => "SHA-256",
            Self::Sha512 => "SHA-512",
            Self::Blake3 => "BLAKE3",
        }
    }

    pub(crate) fn cli_name(&self) -> &'static str {
        match self {
            Self::Md5 => "md5",
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
            Self::Sha512 => "sha512",
            Self::Blake3 => "blake3",
        }
    }
}

struct SupportedHashes {
    md5: String,
    sha1: String,
    sha256: String,
    sha512: String,
    blake3: String,
}

impl SupportedHashes {
    fn for_path(path: &Path) -> io::Result<Self> {
        Ok(Self::from_bytes(&fs::read(path)?))
    }

    fn from_bytes(bytes: &[u8]) -> Self {
        let md5 = {
            let mut hasher = Md5::new();
            hasher.update(bytes);
            format!("{:x}", hasher.finalize())
        };
        let sha1 = {
            let mut hasher = Sha1::new();
            hasher.update(bytes);
            format!("{:x}", hasher.finalize())
        };
        let sha256 = {
            let mut hasher = Sha256::new();
            hasher.update(bytes);
            format!("{:x}", hasher.finalize())
        };
        let sha512 = {
            let mut hasher = Sha512::new();
            hasher.update(bytes);
            format!("{:x}", hasher.finalize())
        };
        let blake3 = blake3::hash(bytes).to_hex().to_string();

        Self {
            md5,
            sha1,
            sha256,
            sha512,
            blake3,
        }
    }

    fn get(&self, algorithm: HashAlgorithm) -> &str {
        match algorithm {
            HashAlgorithm::Md5 => &self.md5,
            HashAlgorithm::Sha1 => &self.sha1,
            HashAlgorithm::Sha256 => &self.sha256,
            HashAlgorithm::Sha512 => &self.sha512,
            HashAlgorithm::Blake3 => &self.blake3,
        }
    }

    fn matches(&self, candidate: &str) -> bool {
        [
            self.md5.as_str(),
            self.sha1.as_str(),
            self.sha256.as_str(),
            self.sha512.as_str(),
            self.blake3.as_str(),
        ]
        .into_iter()
        .any(|hash| hash.eq_ignore_ascii_case(candidate))
    }
}

pub(crate) fn defang_path(
    path: &Path,
    algorithm: HashAlgorithm,
    separator: &str,
) -> io::Result<PathBuf> {
    let hashes = SupportedHashes::for_path(path)?;
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Input path has no file name"))?
        .to_string_lossy();
    let Some(new_file_name) = planned_defanged_name(&file_name, &hashes, algorithm, separator)
    else {
        return Ok(path.to_path_buf());
    };
    rename_file(path, &new_file_name)
}

pub(crate) fn refang_path(
    path: &Path,
    fallback_algorithm: HashAlgorithm,
    separator: &str,
) -> io::Result<PathBuf> {
    let hashes = SupportedHashes::for_path(path)?;
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Input path has no file name"))?
        .to_string_lossy();
    let Some(new_file_name) =
        planned_refanged_name(&file_name, &hashes, fallback_algorithm, separator)
    else {
        return Ok(path.to_path_buf());
    };
    rename_file(path, &new_file_name)
}

fn rename_file(path: &Path, new_file_name: &str) -> io::Result<PathBuf> {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let new_path = parent.join(new_file_name);
    fs::rename(path, &new_path)?;
    Ok(new_path)
}

fn hash_prefix_parts<'a>(
    file_name: &'a str,
    hashes: &SupportedHashes,
    separator: &str,
) -> Option<(&'a str, &'a str)> {
    let (candidate, rest) = file_name.split_once(separator)?;
    if candidate.is_empty() || rest.is_empty() || !hashes.matches(candidate) {
        return None;
    }
    Some((candidate, rest))
}

fn hash_suffix_parts<'a>(
    file_name: &'a str,
    hashes: &SupportedHashes,
    separator: &str,
) -> Option<(&'a str, &'a str)> {
    let (rest, candidate) = file_name.rsplit_once(separator)?;
    if candidate.is_empty() || rest.is_empty() || !hashes.matches(candidate) {
        return None;
    }
    Some((rest, candidate))
}

fn planned_defanged_name(
    file_name: &str,
    hashes: &SupportedHashes,
    algorithm: HashAlgorithm,
    separator: &str,
) -> Option<String> {
    let selected_hash = hashes.get(algorithm);
    let (base_name, had_prefix_hash) = match hash_prefix_parts(file_name, hashes, separator) {
        Some((_, rest)) => (rest, true),
        None => (file_name, false),
    };

    let normalized_base_name = match hash_suffix_parts(base_name, hashes, separator) {
        Some((_, existing_hash))
            if !had_prefix_hash && existing_hash.eq_ignore_ascii_case(selected_hash) =>
        {
            return None;
        }
        Some((rest, _)) => rest,
        None => base_name,
    };

    Some(format!("{normalized_base_name}{separator}{selected_hash}"))
}

fn planned_refanged_name(
    file_name: &str,
    hashes: &SupportedHashes,
    fallback_algorithm: HashAlgorithm,
    separator: &str,
) -> Option<String> {
    if hash_prefix_parts(file_name, hashes, separator).is_some() {
        return None;
    }

    let (base_name, hash) = hash_suffix_parts(file_name, hashes, separator)
        .unwrap_or_else(|| (file_name, hashes.get(fallback_algorithm)));
    Some(format!("{hash}{separator}{base_name}"))
}

#[cfg(test)]
mod tests {
    use super::{
        hash_prefix_parts, hash_suffix_parts, planned_defanged_name, planned_refanged_name,
        HashAlgorithm, SupportedHashes,
    };

    #[test]
    fn defang_detection_matches_hash_at_end() {
        let hashes = SupportedHashes::from_bytes(b"sample");
        let file_name = format!("invoice.exe {}", hashes.get(HashAlgorithm::Sha256));
        assert!(hash_suffix_parts(&file_name, &hashes, " ").is_some());
    }

    #[test]
    fn defang_detection_matches_hash_at_front() {
        let hashes = SupportedHashes::from_bytes(b"sample");
        let file_name = format!("{} invoice.exe", hashes.get(HashAlgorithm::Md5));
        assert!(hash_prefix_parts(&file_name, &hashes, " ").is_some());
    }

    #[test]
    fn defang_detection_ignores_unrelated_hex_text() {
        let hashes = SupportedHashes::from_bytes(b"sample");
        let file_name = "invoice.exe deadbeefdeadbeefdeadbeefdeadbeef";
        assert!(hash_suffix_parts(file_name, &hashes, " ").is_none());
    }

    #[test]
    fn planned_defang_name_appends_selected_hash() {
        let hashes = SupportedHashes::from_bytes(b"sample");
        let file_name = "invoice.exe";
        assert_eq!(
            planned_defanged_name(file_name, &hashes, HashAlgorithm::Sha512, " "),
            Some(format!("{file_name} {}", hashes.get(HashAlgorithm::Sha512)))
        );
    }

    #[test]
    fn planned_defang_name_moves_front_hash_to_end() {
        let hashes = SupportedHashes::from_bytes(b"sample");
        let file_name = format!("{} invoice.exe", hashes.get(HashAlgorithm::Md5));
        assert_eq!(
            planned_defanged_name(&file_name, &hashes, HashAlgorithm::Md5, " "),
            Some(format!("invoice.exe {}", hashes.get(HashAlgorithm::Md5)))
        );
    }

    #[test]
    fn planned_defang_name_replaces_front_hash_with_selected_hash() {
        let hashes = SupportedHashes::from_bytes(b"sample");
        let file_name = format!("{} invoice.exe", hashes.get(HashAlgorithm::Md5));
        assert_eq!(
            planned_defanged_name(&file_name, &hashes, HashAlgorithm::Sha256, " "),
            Some(format!("invoice.exe {}", hashes.get(HashAlgorithm::Sha256)))
        );
    }

    #[test]
    fn planned_defang_name_normalizes_front_and_trailing_hashes() {
        let hashes = SupportedHashes::from_bytes(b"sample");
        let file_name = format!(
            "{} invoice.exe {}",
            hashes.get(HashAlgorithm::Md5),
            hashes.get(HashAlgorithm::Sha1)
        );
        assert_eq!(
            planned_defanged_name(&file_name, &hashes, HashAlgorithm::Sha512, " "),
            Some(format!("invoice.exe {}", hashes.get(HashAlgorithm::Sha512)))
        );
    }

    #[test]
    fn planned_defang_name_keeps_matching_trailing_hash() {
        let hashes = SupportedHashes::from_bytes(b"sample");
        let file_name = format!("invoice.exe {}", hashes.get(HashAlgorithm::Sha256));
        assert_eq!(
            planned_defanged_name(&file_name, &hashes, HashAlgorithm::Sha256, " "),
            None
        );
    }

    #[test]
    fn planned_defang_name_replaces_trailing_hash_with_selected_hash() {
        let hashes = SupportedHashes::from_bytes(b"sample");
        let file_name = format!("invoice.exe {}", hashes.get(HashAlgorithm::Sha256));
        assert_eq!(
            planned_defanged_name(&file_name, &hashes, HashAlgorithm::Md5, " "),
            Some(format!("invoice.exe {}", hashes.get(HashAlgorithm::Md5)))
        );
    }

    #[test]
    fn planned_refang_name_moves_hash_to_front() {
        let hashes = SupportedHashes::from_bytes(b"sample");
        let file_name = format!("invoice.exe {}", hashes.get(HashAlgorithm::Sha256));
        assert_eq!(
            planned_refanged_name(&file_name, &hashes, HashAlgorithm::Md5, " "),
            Some(format!("{} invoice.exe", hashes.get(HashAlgorithm::Sha256)))
        );
    }

    #[test]
    fn planned_refang_name_adds_selected_hash_if_none_matches() {
        let hashes = SupportedHashes::from_bytes(b"sample");
        let file_name = "invoice.exe deadbeefdeadbeefdeadbeefdeadbeef";
        assert_eq!(
            planned_refanged_name(file_name, &hashes, HashAlgorithm::Blake3, " "),
            Some(format!("{} {file_name}", hashes.get(HashAlgorithm::Blake3)))
        );
    }

    #[test]
    fn planned_refang_name_keeps_verified_front_hash() {
        let hashes = SupportedHashes::from_bytes(b"sample");
        let file_name = format!("{} invoice.exe", hashes.get(HashAlgorithm::Md5));
        assert_eq!(
            planned_refanged_name(&file_name, &hashes, HashAlgorithm::Sha256, " "),
            None
        );
    }
}
