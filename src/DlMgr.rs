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

use crate::{FileType, utils};
use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;

pub async fn getVersionInfo(client: &Client, mut ver:String)->Option<String> {
    let json: Value = client
        .get("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")
        .send().await.unwrap()
        .json().await.unwrap();

    if ver.is_empty() {
        ver = json["latest"]["release"].as_str()?.to_string();
    }

    let dlUrl = json["versions"]
        .as_array()?
        .iter()
        .find(|v| v["id"] == ver)?
        ["url"]
        .as_str()?
        .to_string();
    Some(dlUrl)
}

pub async fn getAndHandleInfo(client: &Client, url: String) -> Result<()> {
    let json: Value = client
        .get(url)
        .send().await?
        .json().await?;

    let mut downloaders:Vec<JoinHandle<Result<()>>> = Vec::new();

    let semaphore = Arc::new(Semaphore::new(50));
    let pb = ProgressBar::new(0);
    pb.set_style(ProgressStyle::with_template(
        "[{elapsed_precise}] {bar:40.cyan/blue} {bytes:>7}/{total_bytes:7} ({bytes_per_sec}) {msg}"
    )?.progress_chars("##-"));
    pb.set_message("Downloading...");

    let mcClientUrl = json["downloads"]["client"]["url"].as_str().unwrap().to_string();
    let clientSize = json["downloads"]["client"]["size"].as_u64().unwrap_or(0);
    let version = json["id"].as_str().unwrap().to_string();

    pb.inc_length(clientSize);
    downloaders.push(tokio::spawn(dlFile(client.clone(), mcClientUrl, FileType::Mc(version), semaphore.clone(), pb.clone())));

    let libraries = json["libraries"].as_array().unwrap();

    for lib in libraries {
        let url = lib["downloads"]["artifact"]["url"]
            .as_str()
            .unwrap()
            .to_string();
        let libSize = lib["downloads"]["artifact"]["size"].as_u64().unwrap_or(0);

        let mut isNative = false;
        if let Some(rules) = lib["rules"].as_array() {
            let skip = rules.iter().any(|rule| {
                (rule["action"] == "allow" && rule["os"]["name"]!="windows") || (url.contains("windows-arm64") || url.contains("windows-x86"))
            });
            if skip {
                continue;
            }
            if url.contains("natives") {
                isNative=true;
            }
        }
        pb.inc_length(libSize);
        let dl = tokio::spawn(dlFile(client.clone(), url, if isNative { FileType::Native } else { FileType::Lib } , semaphore.clone(), pb.clone()));
        downloaders.push(dl);
    }

    let assetsIndexUrl = json["assetIndex"]["url"].as_str().unwrap().to_string();
    let indexSize = json["assetIndex"]["size"].as_u64().unwrap_or(0);

    pb.inc_length(indexSize);
    dlFile(client.clone(), assetsIndexUrl, FileType::AssetIndex, semaphore.clone(), pb.clone()).await?;
    utils::getAssets(client.clone(),&mut downloaders, semaphore.clone(), pb.clone()).await?;

    for dl in downloaders {
        dl.await??;
    }

    pb.finish_with_message("Download complete!");
    Ok(())
}

pub async fn dlFile(client: Client, url: String, ft: FileType, semaphore: Arc<Semaphore>, pb: ProgressBar) -> Result<()> {
    let _permit = semaphore.acquire().await?;

    let mut response = client.get(&url).send().await?;
    let raw_filename = url.rsplit('/').next().unwrap_or("file");
    let mut path = PathBuf::new();

    match ft {
        FileType::Lib => {
            path.push("libraries");
            path.push(raw_filename);
        }
        FileType::Native => {
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await? {
                bytes.extend_from_slice(&chunk);
                pb.inc(chunk.len() as u64); // Update progress byte-by-byte
            }
            utils::extract_native(bytes).await?;
            return Ok(());
        }
        FileType::AssetIndex => {
            path.push("assets");
            path.push("indexes");
            path.push(raw_filename);
        }
        FileType::Asset => {
            path.push("assets");
            path.push("objects");
            let subfolder = raw_filename.get(..2).unwrap();
            path.push(subfolder);
            path.push(raw_filename);
        }
        FileType::Mc(ver) => {
            let base = raw_filename.split(".").next().unwrap();
            path.push(format!("{ver}-{base}.jar"));
        }
    }

    let mut file = tokio::fs::File::create(path).await?;

    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk).await?;
        pb.inc(chunk.len() as u64);
    }
    Ok(())
}