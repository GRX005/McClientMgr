use reqwest::Response;
use std::fs::File;
use std::io::{Cursor, Error};
use std::path::Path;
use std::{fs, io};
use zip::ZipArchive;

pub fn makeFolders() ->Result<(), Error> {
    fs::create_dir_all("libraries")?;
    fs::create_dir_all("natives")?;
    fs::create_dir_all("assets")?;
    Ok(())
}

pub async fn extract_native(response: Response) -> anyhow::Result<()> {
    let bytes = response.bytes().await?;
    let out_dir = Path::new("natives/");

    tokio::task::spawn_blocking(move || {
        let mut archive = ZipArchive::new(Cursor::new(bytes))?;

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let name = entry.name();
            if entry.is_dir() || name.starts_with("META-INF/") {
                continue;
            }
            let is_native = matches!(
                Path::new(name).extension().and_then(|e| e.to_str()),
                Some("dll") | Some("so") | Some("dylib")
            );
            if !is_native {
                continue;
            }
            let file_name = Path::new(name).file_name().unwrap();
            let mut out_file = File::create(out_dir.join(file_name))?;
            io::copy(&mut entry, &mut out_file)?;
        }
        Ok(())
    }).await?
}