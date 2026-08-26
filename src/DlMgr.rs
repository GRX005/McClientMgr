use crate::{FileType, utils};
use anyhow::{Error, Result};
use reqwest::Client;
use serde_json::Value;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tokio::task::JoinHandle;

pub async fn getLatestVer(client: &Client) -> Result<(String,String), Error> {
    let json: Value = client
        .get("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")
        .send().await?
        .json().await?;

    let latest = json["latest"]["release"].as_str().unwrap().to_string();

    let dlUrl = json["versions"]
        .as_array().unwrap()
        .iter()
        .find(|v| v["id"] == latest)
        .unwrap()["url"]
        .as_str().unwrap()
        .to_string();
    Ok((latest, dlUrl))
}

pub async fn getAndHandleInfo(client: &Client, url: String) -> Result<()> {
    let json: Value = client
        .get(url)
        .send().await?
        .json().await?;

    let mut downloaders:Vec<JoinHandle<Result<()>>> = Vec::new();

    let mcClientUrl = json["downloads"]["client"]["url"].as_str().unwrap().to_string();
    let version = json["id"].as_str().unwrap().to_string();

    downloaders.push(tokio::spawn(dlFile(client.clone(), mcClientUrl, FileType::Mc(version))));

    let libraries = json["libraries"].as_array().unwrap();

    for lib in libraries {
        let url = lib["downloads"]["artifact"]["url"]
            .as_str()
            .unwrap()
            .to_string();

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

        let dl = tokio::spawn(dlFile(client.clone(), url, if isNative { FileType::Native } else { FileType::Lib } ));
        downloaders.push(dl);
    }

    let assetsIndexUrl = json["assetIndex"]["url"].as_str().unwrap().to_string();
    dlFile(client.clone(), assetsIndexUrl, FileType::AssetIndex).await?;
    utils::getAssets(client.clone(),&mut downloaders).await?;

    for dl in downloaders {
        dl.await??;
    }
    Ok(())

}

pub async fn dlFile(client: Client, url: String, ft: FileType) -> Result<()> {
    let mut response = client.get(&url).send().await?;

    let raw_filename = url.rsplit('/').next().unwrap_or("file");
    let mut path = PathBuf::new();

    match ft {
        FileType::Lib => {
            path.push("libraries");
            path.push(raw_filename);
        }
        FileType::Native => {
            utils::extract_native(response).await?;
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
    }
    Ok(())
}