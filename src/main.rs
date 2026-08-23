#![allow(non_snake_case)]

mod DlMgr;
mod utils;

use anyhow::{Context, bail};
use reqwest::{Client, ClientBuilder, tls};
use std::env;
use std::process::Command;
use tokio::try_join;
use uuid::Uuid;

#[derive(PartialEq)]
pub enum FileType {
    Lib,
    Native,
    Asset,
    AssetIndex,
    Mc
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Minecraft Client Manager v{}", env!("CARGO_PKG_VERSION"));
    let client = getWebClient();
    let (latVerAndUrl, _) = try_join!(
        DlMgr::getLatestVer(&client),
        utils::makeFolders()
    )?;
    println!("Latest version: {}; URL: {}",latVerAndUrl.0,latVerAndUrl.1);
    DlMgr::getAndHandleInfo(&client, latVerAndUrl.1).await?;
    launchGame()?;
    Ok(())
}

fn getWebClient() -> Client {
    ClientBuilder::new().user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"))).min_tls_version(tls::Version::TLS_1_3).https_only(true).build().unwrap()
}

fn launchGame()->anyhow::Result<()> {
    let current_dir = env::current_dir().context("Failed to get current directory")?;

    let natives = current_dir.join("natives");
    let game_dir = current_dir.join("MC");
    let assets_dir = current_dir.join("assets");
    let cp_sep = if cfg!(windows) { ";" } else { ":" };

    let class_path = format!(
        "{}{cp_sep}{}",
        current_dir.join("libraries").join("*").display(),
        current_dir.join("client.jar").display()
    );

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
            "-Dminecraft.launcher.version=1.7",
            "-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump",
            "-Xss1M",
        ])
        .args([
            // native library system properties
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
            "--version", "26.2",
            "--versionType", LAUNCHER_BRAND,
        ])
        .arg("--gameDir").arg(game_dir)
        .arg("--assetsDir").arg(assets_dir)
        .args(["--assetIndex", "32"])
        .args(["--accessToken", LAUNCHER_BRAND])
        .status()?;

    if !status.success(){
        bail!("java exited with {status}");
    }

    Ok(())
}

