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

#![allow(non_snake_case)]

mod DlMgr;
mod utils;

use anyhow::bail;
use reqwest::{Client, ClientBuilder, tls};
use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;
use std::{env, fs};
use uuid::Uuid;

#[derive(PartialEq)]
pub enum FileType {
    Lib,
    Native,
    Asset,
    AssetIndex,
    Mc(String)
}
const VERSION:&str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Minecraft Client Manager v{}", VERSION);
    if let Some(clientName) = getMcClient() {
        launchGame(clientName)?;
        return Ok(())
    }

    let client = getWebClient();
    let url = loop {
        let ver = utils::getVer()?;
        if let Some(v) = DlMgr::getVersionInfo(&client, ver).await {
            break v;
        }
        println!("Invalid version.");
    };

    utils::makeFolders().await?;
    println!("URL: {}",url);

    DlMgr::getAndHandleInfo(&client, url).await?;
    launchGame(getMcClient().unwrap())?;
    Ok(())
}

fn getWebClient() -> Client {
    ClientBuilder::new().user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"))).min_tls_version(tls::Version::TLS_1_3).https_only(true).build().unwrap()
}

fn launchGame(clientName:String)->anyhow::Result<()> {
    let assetIndex = getAssetIndex()?;
    let clientVersion = clientName.split("-").next().unwrap();
    println!("Launcing MC version {} with assetIndex {}...", clientVersion, assetIndex);

    let current_dir = env::current_dir()?;

    let cp_sep = if cfg!(windows) { ";" } else { ":" };
    let class_path = format!(
        "{}{cp_sep}{}",
        current_dir.join("libraries").join("*").display(),
        current_dir.join(&clientName).display()
    );

    let natives = current_dir.join("natives");
    let game_dir = current_dir.join("MC");
    let assets_dir = current_dir.join("assets");

    const LAUNCHER_BRAND: &str = "McClientMgr";

    let status = Command::new("java")
        .args([
            // memory / JVM tuning
            "-Xms2G",
            "-Xmx4G",
            "--sun-misc-unsafe-memory-access=allow",
            "--enable-native-access=ALL-UNNAMED",
            "-XX:+UseCompactObjectHeaders",
            "-XX:+AlwaysPreTouch",
            "-XX:+UseStringDeduplication",
            "-XX:+UseZGC",
            "-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump",
            "-Xss1M",
        ])
        .args([
            // native library system properties
            format!("-Dminecraft.launcher.version={}", VERSION),
            format!("-Djava.library.path={}", natives.display()),
            format!("-Djna.tmpdir={}", natives.display()),
            format!("-Dorg.lwjgl.system.SharedLibraryExtractPath={}", natives.display()),
            format!("-Dio.netty.native.workdir={}", natives.display()),
            format!("-Dminecraft.launcher.brand={}", LAUNCHER_BRAND),
        ])
        .args(["-cp", &class_path])
        .arg("net.minecraft.client.main.Main")
        .args([
            // identity args
            "--uuid", &Uuid::new_v4().to_string(),
            "--clientId", LAUNCHER_BRAND,
            "--xuid", LAUNCHER_BRAND,
            "--versionType", "release",
            "--accessToken", LAUNCHER_BRAND,
        ])
        .arg("--gameDir").arg(game_dir)
        .arg("--assetsDir").arg(assets_dir)
        .arg("--assetIndex").arg(assetIndex)
        .arg("--version").arg(clientVersion)
        .status()?;

    if !status.success(){
        bail!("java exited with {status}");
    }

    Ok(())
}

fn getAssetIndex() -> anyhow::Result<String> {
    let entry = fs::read_dir("assets/indexes/")?.next().unwrap()?.file_name();
    let name = entry.to_string_lossy().split(".").next().unwrap().to_string();
    Ok(name)
}

fn getMcClient() -> Option<String> {
    let jarName = fs::read_dir(".").unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name())
        .find(|name| Path::new(name).extension() == Some(OsStr::new("jar")))?;
    Some(jarName.to_string_lossy().into_owned())
}