#![allow(non_snake_case)]

mod DlMgr;
mod utils;

use reqwest::{Client, ClientBuilder, tls};
use tokio::try_join;

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
    Ok(())
}

fn getWebClient() -> Client {
    ClientBuilder::new().user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"))).min_tls_version(tls::Version::TLS_1_3).https_only(true).build().unwrap()
}

