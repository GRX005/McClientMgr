/*
    This file is part of the McClientMgr project, licensed under the
    GNU General Public License v3.0

    Copyright (C) 2026 _1ms (GRX005)

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program. If not, see <https://www.gnu.org/licenses/>.
*/

use crate::{DlMgr, FileType};
use anyhow::Result;
use indicatif::ProgressBar;
use reqwest::Client;
use serde_json::Value;
use std::fs::File;
use std::io::{Cursor, Write, stdin, stdout};
use std::path::Path;
use std::sync::Arc;
use std::{fs, io};
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use zip::ZipArchive;
use crate::DlMgr::dlFile;

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

pub async fn extract_native(bytes: Vec<u8>) -> Result<()> {
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

pub async fn getAssets(client: Client, downloaders: &mut Vec<JoinHandle<Result<()>>>, semaphore: Arc<Semaphore>, pb: ProgressBar) -> Result<()> {
    let entry = tokio::fs::read_dir("assets/indexes/").await?.next_entry().await?.unwrap();
    let ass = tokio::fs::read_to_string(entry.path()).await?;

    let json: Value = serde_json::from_str(&ass)?;
    let objs = json["objects"].as_object().unwrap();

    let total_assets_size: u64 = objs.values().map(|obj| obj["size"].as_u64().unwrap_or(0)).sum();
    pb.inc_length(total_assets_size);

    for (_, obj) in objs {
        let obj_hash = obj["hash"].as_str().unwrap();
        let url = format!("https://resources.download.minecraft.net/{}/{}", &obj_hash[..2], obj_hash);

        downloaders.push(tokio::spawn(DlMgr::dlFile(client.clone(), url, FileType::Asset, semaphore.clone(), pb.clone())))
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

pub fn getLibraries(client: Client, downloaders: &mut Vec<JoinHandle<Result<()>>>, semaphore: Arc<Semaphore>, pb: ProgressBar, json: &Value) {
    let libraries = json["libraries"].as_array().unwrap();

    for lib in libraries {
        let url = lib["downloads"]["artifact"]["url"]
            .as_str()
            .unwrap()
            .to_string();
        let libSize = lib["downloads"]["artifact"]["size"].as_u64().unwrap_or(0);

        let mut nativeNeedExtract = false;
        if let Some(rules) = lib["rules"].as_array() {
            let skip = rules.iter().any(|rule| {
                (rule["action"] == "allow" && rule["os"]["name"]!="windows") || (url.contains("windows-arm64") || url.contains("windows-x86"))
            });
            if skip {
                continue;
            }
            if url.contains("natives") {
                nativeNeedExtract =true;
            }
        }
        pb.inc_length(libSize);
        let dl = tokio::spawn(dlFile(client.clone(), url, if nativeNeedExtract { FileType::Native } else { FileType::Lib }, semaphore.clone(), pb.clone()));
        downloaders.push(dl);
    }
}