use crate::{DlMgr, FileType};
use anyhow::{Error, Result};
use reqwest::{Client, Response};
use serde_json::Value;
use std::fs::File;
use std::io::{stdout, Cursor, Write, stdin};
use std::path::Path;
use std::{fs, io};
use tokio::task::JoinHandle;
use zip::ZipArchive;

pub async fn makeFolders() -> Result<()> {
    tokio::task::spawn_blocking(|| {
        fs::create_dir_all("libraries")?;
        fs::create_dir_all("natives")?;
        fs::create_dir_all("assets/indexes")?;
        fs::create_dir_all("assets/objects")?;

        for i in 0u8..=255 {
            fs::create_dir_all(format!("assets/objects/{:02x}", i))?;
        }

        Ok(())
    }).await?
}

pub async fn extract_native(response: Response) -> Result<()> {
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

pub async fn getAssets(client: Client, downloaders: &mut Vec<JoinHandle<Result<()>>>) -> Result<()> {
    let entry = tokio::fs::read_dir("assets/indexes/").await?.next_entry().await?.unwrap();
    let ass = tokio::fs::read_to_string(entry.path()).await?;

    let json: Value = serde_json::from_str(&ass)?;
    let objs = json["objects"].as_object().unwrap();

    for (_, obj) in objs {
        let obj_hash = obj["hash"].as_str().unwrap();
        let url = format!("https://resources.download.minecraft.net/{}/{}", &obj_hash[..2], obj_hash);

        downloaders.push(tokio::spawn(DlMgr::dlFile(client.clone(), url, FileType::Asset)))
    }

    Ok(())

}

pub fn getVer()->Result<String> {
    let mut input;
    loop {
        print!("Version to download ({}): ","latest");
        stdout().flush()?;
        input = String::new();
        stdin().read_line(&mut input)?;
        input = input.trim().to_string();
        if input.bytes().all(|b| matches!(b, b'0'..=b'9' | b'.' | b'-' | b'a'..=b'z' | b'A'..=b'Z')) {
            break
        }
        println!("Invalid character!");
    }
    println!("Getting version information...");
    Ok(input)
}