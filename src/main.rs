#![allow(non_snake_case)]

mod DlMgr;
mod utils;

use reqwest::{Client, ClientBuilder, tls};

#[tokio::main]
async fn main() -> anyhow::Result<(), anyhow::Error> {
    println!("Minecraft Client Manager v{}", env!("CARGO_PKG_VERSION"));

    let client = getWebClient();
    let latVerAndUrl = DlMgr::getLatestVer(&client).await?;
    utils::makeFolders()?;
    println!("{}",latVerAndUrl.1);
    DlMgr::getAndHandleInfo(&client, latVerAndUrl.1).await?;
    Ok(())

    
}

fn getWebClient() -> Client {
    ClientBuilder::new().user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"))).min_tls_version(tls::Version::TLS_1_3).https_only(true).build().unwrap()
}

