use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

pub(super) fn check_identity(manifest_path: &Path, max_risk: &str) -> io::Result<String> {
    let manifest_path = manifest_path.canonicalize()?;
    let config =
        axion_manifest::load_app_config_from_path(&manifest_path).map_err(io::Error::other)?;
    let mut digest = Sha256::new();
    hash_value(&mut digest, b"axion.check-identity.v1");
    hash_value(&mut digest, env!("CARGO_PKG_VERSION").as_bytes());
    hash_value(&mut digest, max_risk.as_bytes());
    hash_value(&mut digest, b"deny-warnings=true;bundle=true");
    hash_value(&mut digest, manifest_path.to_string_lossy().as_bytes());
    hash_file(&mut digest, &manifest_path)?;
    hash_tree(
        &mut digest,
        &config.build.frontend_dist,
        &config.build.frontend_dist,
    )?;
    if let Some(icon) = config.bundle.icon {
        hash_value(&mut digest, b"icon");
        hash_file(&mut digest, &icon)?;
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn hash_value(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_le_bytes());
    digest.update(value);
}

fn hash_file(digest: &mut Sha256, path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() {
        return Err(io::Error::other("check inputs must be regular files"));
    }
    digest.update(metadata.len().to_le_bytes());
    let mut file = fs::File::open(path)?;
    let mut buffer = [0; 8192];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(())
}

fn hash_tree(digest: &mut Sha256, root: &Path, path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    let relative = path.strip_prefix(root).map_err(io::Error::other)?;
    hash_value(digest, relative.to_string_lossy().as_bytes());
    if metadata.is_file() {
        hash_value(digest, b"file");
        hash_file(digest, path)?;
    } else if metadata.is_dir() {
        hash_value(digest, b"directory");
        let mut children = fs::read_dir(path)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<io::Result<Vec<PathBuf>>>()?;
        children.sort();
        for child in children {
            hash_tree(digest, root, &child)?;
        }
    } else {
        return Err(io::Error::other(
            "check frontend contains a symlink or unsupported file type",
        ));
    }
    Ok(())
}
