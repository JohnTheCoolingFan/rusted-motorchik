use std::error::Error;
use thiserror::Error;
use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::builder::{Timestamp, CreateEmbed};
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::{CommandResult, Args};
use scraper::{Html, Selector};
use reqwest::{self, StatusCode};
use serde::Deserialize;
use semver::Version;

// TODO: Re-use client session

pub const MOD_LIST: [&str; 6] = ["artillery-spidertron", "PlaceableOffGrid", "NoArtilleryMapReveal", "RandomFactorioThings", "PlutoniumEnergy", "ReactorDansen"];

const MODPORTAL_URL: &str = "https://mods.factorio.com";
const LAUNCHER_URL: &str = "https://factorio-launcher-mods.storage.googleapis.com/";

const FAILED_EMBED_COLOR: (u8, u8, u8) = (255, 10, 10);
const SUCCESS_EMBED_COLOR: (u8, u8, u8) = (47, 137, 197);

pub struct FactorioReqwestClient;

impl TypeMapKey for FactorioReqwestClient {
    type Value = reqwest::Client;
}

struct ModData {
    title: String,
    description: String,
    url: String,
    timestamp: Option<Timestamp>,
    color: (u8, u8, u8),
    thumbnail_url: Option<String>,
    game_version: Option<String>,
    download: Option<ModDownload>,
    latest_version: Option<String>,
    downloads_count: usize,
    author: String,
}

impl ModData {
    pub fn new(title: String, description: String, url: String, color: (u8, u8, u8), downloads_count: usize, author: String) -> Self {
        ModData{title, description, url, color, downloads_count, author,
            timestamp: None, thumbnail_url: None, game_version: None, download: None, latest_version: None}
    }
}

struct ModDownload {
    official: String,
    launcher: String,
}

#[derive(Deserialize)]
struct ModInfo {
    releases: Vec<ModRelease>,
    thumbnail: String,
    title: String,
    summary: String,
    downloads_count: usize,
    owner: String,
}

#[derive(Deserialize)]
struct ModRelease {
    version: Version,
    info_json: InfoJson,
    released_at: String,
    download_url: String,
}

#[derive(Deserialize)]
struct InfoJson {
    factorio_version: String
}

#[command]
#[aliases(modstat, ms)]
async fn mods_statistics(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    for mod_name in args.quoted().trimmed().iter::<String>() {
        process_mod(ctx, msg, &mod_name?).await?;
    }
    Ok(())
}

#[command]
#[aliases(ml)]
async fn modlist(ctx: &Context, msg: &Message) -> CommandResult {
    for mod_name in MOD_LIST {
        process_mod(ctx, msg, mod_name).await?;
    }
    Ok(())
}

async fn process_mod(ctx: &Context, msg: &Message, mod_name: &str) -> CommandResult {
    let mod_data = get_mod_info(ctx, mod_name).await;
    msg.channel_id.send_message(&ctx.http, |m| m.embed(|e| {
        match mod_data {
            Ok(data) => construct_mod_embed(e, data),
            Err(_) => e.title("Mod not found")
                .description(format!("Failed to find {}", mod_name))
                .color(FAILED_EMBED_COLOR)
        }
    })).await?;
    Ok(())
}

fn construct_mod_embed(e: &mut CreateEmbed, data: ModData) -> &mut CreateEmbed {
    e.title(data.title)
        .description(data.description)
        .url(data.url)
        .color(data.color);
    if let Some(game_version) = data.game_version {
        e.field("Game version", game_version, true);
    }
    if let Some(download) = data.download {
        e .field("Download", format!("[From official mod portal]({})\n[From Factorio Launcher storage]({})",
                download.official, download.launcher), true);
    }
    if let Some(latest_version) = data.latest_version {
        e.field("Latest version", latest_version, true);
    }
    e.field("Downloaded", format!("{} times", data.downloads_count), true);
    e.field("Author", format!("[{0}](https://mods.factorio.com/user/{0})", data.author), true);
    if let Some(timestamp) = data.timestamp {
        e.timestamp(timestamp).footer(|f| f.text("Latest version was released at:"));
    }
    if let Some(thumbnail_url) = data.thumbnail_url {
        e.thumbnail(thumbnail_url);
    }
    e
}

async fn get_mod_info(ctx: &Context, mod_name: &str) -> Result<ModData, Box<dyn Error + Send + Sync>> {
    let client_data = ctx.data.read().await;
    let reqwest_client = client_data.get::<FactorioReqwestClient>().unwrap();
    let api_response = reqwest_client.get(format!("{}/api/mods/{}", MODPORTAL_URL, mod_name)).send().await?;
    if api_response.status() == StatusCode::OK {
        parse_mod_data(api_response, mod_name).await
    } else {
        let new_mod_name = find_mod(ctx, mod_name).await;
        if let Ok(mname) = new_mod_name {
            let api_response = reqwest::get(format!("{}/api/mods/{}", MODPORTAL_URL, &mname)).await?;
            if api_response.status() == StatusCode::OK {
                return parse_mod_data(api_response, &mname).await
            }
        }
        Err(ModError::NotFound.into())
    }
}

async fn parse_mod_data(api_response: reqwest::Response, mod_name: &str) -> Result<ModData, Box<dyn Error + Send + Sync>> {
    let mut mod_info: ModInfo = api_response.json().await?;
    mod_info.releases.sort_by(|rls1, rls2| rls1.version.cmp(&rls2.version));
    let latest_release = mod_info.releases.last();
    let mut result = ModData::new(
        mod_info.title,
        mod_info.summary,
        format!("{}/mod/{}", MODPORTAL_URL, mod_name),
        SUCCESS_EMBED_COLOR,
        mod_info.downloads_count,
        mod_info.owner,
    );
    if let Some(lrls) = latest_release {
        result.timestamp = Some(lrls.released_at.clone().into());
        result.game_version = Some(lrls.info_json.factorio_version.clone());
        result.download = Some(ModDownload{official: format!("{}{}", MODPORTAL_URL, lrls.download_url),
            launcher: format!("{}/{}/{}.zip", LAUNCHER_URL, mod_name, lrls.version)});
        result.latest_version = Some(lrls.version.to_string());
    }
    if mod_info.thumbnail != "/assets/.thumb.png" {
        result.thumbnail_url = format!("https://mods-data.factorio.com{}", mod_info.thumbnail).into();
    }
    Ok(result)
}

async fn find_mod(ctx: &Context, mod_name: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
    let client_data = ctx.data.read().await;
    let reqwest_client = client_data.get::<FactorioReqwestClient>().unwrap();
    let search_response = reqwest_client.get(format!("{}/query/{}", MODPORTAL_URL, mod_name)).send().await?;
    if search_response.status() == StatusCode::OK {
        let selector = Selector::parse("h2.mb0").unwrap();
        let document = Html::parse_document(&search_response.text().await?);
        match document.select(&selector).next() {
            Some(elem) => {
                let asel = Selector::parse("a").unwrap();
                match elem.select(&asel).next() {
                    Some(mod_link) => match mod_link.value().attr("href") {
                        Some(link) => Ok(String::from(&link[5..])),
                        None => Err(ModError::NotFound.into())
                    },
                    None => Err(ModError::NotFound.into())
                }
            },
            None => Err(ModError::NotFound.into())
        }
    } else {
        Err(ModError::NotFound.into())
    }
}

#[group]
#[commands(mods_statistics, modlist)]
struct Factorio;

#[derive(Error, Debug)]
pub enum ModError {
    #[error("Mod not found")]
    NotFound
}
