use reqwest::{Client, Error};
use serde_json::Value;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use anyhow::Result;
use tokio::join;
use tokio::task::JoinHandle;

enum FileType {
    Lib,
    Native,
    Asset,
    Mc
}

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

    let assetID = &json["assets"].as_i64();
    let clientUrl = json["downloads"]["client"]["url"].as_str().unwrap().to_string();
    let dl = tokio::spawn(dlFile(client.clone(), clientUrl, FileType::Mc));
    downloaders.push(dl);

    let libraries = json["libraries"].as_array().unwrap();

    for lib in libraries {
        let url = lib["downloads"]["artifact"]["url"]
            .as_str()
            .unwrap()
            .to_string();

        if let Some(rules) = lib["rules"].as_array() {
            let skip = rules.iter().any(|rule| {
                (rule["action"] == "allow" && rule["os"]["name"]!="windows") || (url.contains("windows-arm64") || url.contains("windows-x86"))
            });
            if skip {
                continue;
            }
        }

        let dl = tokio::spawn(dlFile(client.clone(), url, FileType::Lib));
        downloaders.push(dl);
    }

    for dl in downloaders {
        dl.await??;
    }
    Ok(())

}

async fn dlFile(client: Client, url: String, ft: FileType)->Result<()> {
    let mut response = client.get(&url).send().await?;

    let folder = match ft {
        FileType::Lib =>"libraries/",
        FileType::Native =>"natives/",
        FileType::Asset=>"assets/",
        FileType::Mc=>""
    };
    let path = folder.to_owned()+url.split('/').last().unwrap_or("file");

    let mut file = File::create(path).await?;

    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk).await?;
    }
    Ok(())
}