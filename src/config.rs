use std::{
    env, fs, io,
    io::Write,
    path::{Path, PathBuf},
};

use clap::ValueEnum;

use crate::rename::HashAlgorithm;

fn config_path() -> io::Result<PathBuf> {
    #[cfg(target_os = "windows")]
    let (variable, relative) = ("APPDATA", "MalFang_tool/config.toml");
    #[cfg(target_os = "linux")]
    let (variable, relative) = if env::var_os("XDG_CONFIG_HOME").is_some_and(|v| !v.is_empty()) {
        ("XDG_CONFIG_HOME", "malfang-tool/config.toml")
    } else {
        ("HOME", ".config/malfang-tool/config.toml")
    };

    let base = env::var_os(variable)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{variable} must be set to an absolute path to locate the user config"),
            )
        })?;
    Ok(base.join(relative))
}

fn read_config(path: &Path) -> io::Result<toml::Table> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(toml::Table::new()),
        Err(err) => {
            return Err(io::Error::new(
                err.kind(),
                format!("Could not read {}: {err}", path.display()),
            ))
        }
    };
    contents.parse().map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid config {}: {err}", path.display()),
        )
    })
}

fn load_default_at(path: &Path) -> io::Result<HashAlgorithm> {
    let config = read_config(path)?;
    let Some(value) = config.get("default_hash") else {
        return Ok(HashAlgorithm::Sha256);
    };
    let algorithm = value
        .as_str()
        .and_then(|value| HashAlgorithm::from_str(value, true).ok());
    algorithm.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Invalid default_hash in {}: expected md5, sha1, sha256, sha512, or blake3",
                path.display()
            ),
        )
    })
}

pub(crate) fn default_hash() -> io::Result<HashAlgorithm> {
    load_default_at(&config_path()?)
}

fn save_default_at(path: &Path, algorithm: HashAlgorithm) -> io::Result<()> {
    // Preserve unrelated settings and do not overwrite malformed TOML.
    let mut config = read_config(path)?;
    config.insert(
        "default_hash".into(),
        toml::Value::String(algorithm.cli_name().into()),
    );
    let contents = toml::to_string_pretty(&config).map_err(io::Error::other)?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Config path has no parent"))?;
    fs::create_dir_all(parent)?;
    // Write beside the destination, then atomically replace it (also on Windows).
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(contents.as_bytes())?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|err| err.error)?;
    Ok(())
}

pub(crate) fn set_default_hash(algorithm: HashAlgorithm) -> io::Result<PathBuf> {
    let path = config_path()?;
    save_default_at(&path, algorithm)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_config_or_setting_uses_sha256() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        assert_eq!(load_default_at(&path).unwrap(), HashAlgorithm::Sha256);
        fs::write(&path, "other = true\n").unwrap();
        assert_eq!(load_default_at(&path).unwrap(), HashAlgorithm::Sha256);
    }

    #[test]
    fn saves_each_algorithm_and_preserves_other_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/config.toml");
        for algorithm in [
            HashAlgorithm::Md5,
            HashAlgorithm::Sha1,
            HashAlgorithm::Sha256,
            HashAlgorithm::Sha512,
            HashAlgorithm::Blake3,
        ] {
            save_default_at(&path, algorithm).unwrap();
            assert_eq!(load_default_at(&path).unwrap(), algorithm);
        }
        fs::write(&path, "default_hash = 'md5'\nother = true\n").unwrap();
        save_default_at(&path, HashAlgorithm::Sha256).unwrap();
        assert_eq!(read_config(&path).unwrap()["other"].as_bool(), Some(true));
        assert_eq!(load_default_at(&path).unwrap(), HashAlgorithm::Sha256);
    }

    #[test]
    fn rejects_invalid_defaults_and_malformed_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        for contents in [
            "default_hash = 'invalid'",
            "default_hash = 123",
            "not valid toml",
        ] {
            fs::write(&path, contents).unwrap();
            assert_eq!(
                load_default_at(&path).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
        assert!(save_default_at(&path, HashAlgorithm::Md5).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "not valid toml");
    }
}
