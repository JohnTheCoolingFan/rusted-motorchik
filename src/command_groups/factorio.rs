use reqwest::{self, StatusCode};
use scraper::{Html, Selector};
use semver::Version;
use serde::Deserialize;
use serenity::{
    builder::CreateEmbed,
    framework::standard::{
        macros::{command, group},
        {Args, CommandError, CommandResult},
    },
    model::{prelude::*, timestamp::Timestamp},
    prelude::*,
    utils::ArgumentConvert,
};
use std::error::Error;
use std::sync::Arc;
use thiserror::Error;

use crate::guild_config::{GuildConfig, GuildConfigManager, InfoChannelType};

pub const MOD_LIST: [&str; 6] = [
    "artillery-spidertron",
    "PlaceableOffGrid",
    "NoArtilleryMapReveal",
    "RandomFactorioThings",
    "PlutoniumEnergy",
    "ReactorDansen",
];

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
    pub fn new(
        title: String,
        description: String,
        url: String,
        color: (u8, u8, u8),
        downloads_count: usize,
        author: String,
    ) -> Self {
        ModData {
            title,
            description,
            url,
            color,
            downloads_count,
            author,
            timestamp: None,
            thumbnail_url: None,
            game_version: None,
            download: None,
            latest_version: None,
        }
    }

    pub fn result_to_embed<'a>(
        embed: &'a mut CreateEmbed,
        mod_data: Result<ModData, Box<dyn Error + Send + Sync>>,
        mod_name: &str,
    ) -> &'a mut CreateEmbed {
        match mod_data {
            Ok(data) => construct_mod_embed(embed, data),
            Err(_) => embed
                .title("Mod not found")
                .description(format!("Failed to find {mod_name}"))
                .color(FAILED_EMBED_COLOR),
        }
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
    factorio_version: String,
}

/// Get info about a Factorio mod from official mod portal.
#[command]
#[aliases(modstat, ms)]
#[example("Plutonium Energy, Random Factorio Things, Krastorio")]
#[usage("mod name[, mod name, ...]")]
#[min_args(1)]
async fn mods_statistics(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    for mod_name in args.quoted().trimmed().iter::<String>() {
        process_mod(ctx, msg.channel_id, &mod_name?).await?;
    }
    Ok(())
}

/// Change mod-list info channel mods
#[command]
#[aliases(ml)]
#[usage("mod name[, mod name, ...]")]
#[example("Random Factorio Things, Krastorio 2")]
#[only_in(guilds)]
async fn modlist(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let guild_config =
        GuildConfigManager::get_guild_config_from_ctx(ctx, msg.guild_id.unwrap()).await?;
    let guild_config_read = guild_config.read().await;
    let channel = {
        let modlist_ic_data = guild_config_read.info_channels_data(InfoChannelType::ModList);
        let channel = modlist_ic_data.read().await.channel_id;
        channel
    };
    let mod_list: Result<Vec<String>, _> =
        Result::from_iter(args.quoted().trimmed().iter::<String>());
    let mod_list = mod_list?;
    for (_, message_id) in &*guild_config_read.mod_list_messages.read().await {
        let message = Message::convert(
            ctx,
            Some(msg.guild_id.unwrap()),
            Some(channel),
            &message_id.to_string(),
        )
        .await?;
        message.delete(ctx).await?
    }
    {
        let mut mlm_write = guild_config_read.mod_list_messages.write().await;
        mlm_write.clear();
        let mod_list_messages = scheduled_modlist(ctx, channel, mod_list).await?;
        mlm_write.extend(mod_list_messages);
    }
    guild_config_read.write().await?;
    Ok(())
}

pub async fn scheduled_modlist<T: AsRef<str>>(
    ctx: &Context,
    channel: ChannelId,
    mod_list: impl AsRef<[T]>,
) -> std::result::Result<Vec<(String, MessageId)>, CommandError> {
    let mut result = Vec::new();
    for mod_name in mod_list.as_ref() {
        result.push((
            mod_name.as_ref().into(),
            process_mod(ctx, channel, mod_name.as_ref()).await?,
        ));
    }
    Ok(result)
}

async fn process_mod(
    ctx: &Context,
    channel: ChannelId,
    mod_name: &str,
) -> std::result::Result<MessageId, CommandError> {
    let mod_data = get_mod_info(ctx, mod_name).await;
    Ok(channel
        .send_message(&ctx.http, |m| {
            m.embed(|e| ModData::result_to_embed(e, mod_data, mod_name))
        })
        .await?
        .id)
}

pub async fn reply_process_mod(ctx: &Context, msg: &Message, mod_name: &str) -> CommandResult {
    let mod_data = get_mod_info(ctx, mod_name).await;
    msg.channel_id
        .send_message(ctx, |cm| {
            cm.reference_message(msg)
                .embed(|ce| ModData::result_to_embed(ce, mod_data, mod_name))
        })
        .await?;
    Ok(())
}

pub async fn edit_update_mod_list(
    ctx: &Context,
    channel: ChannelId,
    guild: GuildId,
    messages: Arc<RwLock<Vec<(String, MessageId)>>>,
) -> CommandResult {
    for (mod_name, message_id) in &*messages.read().await {
        let message_id = message_id.to_string();
        let mut message = Message::convert(ctx, Some(guild), Some(channel), &message_id).await?;
        let mod_data = get_mod_info(ctx, mod_name).await;
        message
            .edit(&ctx.http, |ed| {
                ed.embed(|e| ModData::result_to_embed(e, mod_data, mod_name))
            })
            .await?;
    }
    Ok(())
}

pub async fn update_mod_list(
    ctx: &Context,
    guild: GuildId,
    guild_config: Arc<RwLock<GuildConfig>>,
) -> CommandResult {
    let mod_list_ic_data_arc = guild_config
        .read()
        .await
        .info_channels_data(InfoChannelType::ModList);
    let mod_list_ic_data = mod_list_ic_data_arc.read().await;
    if mod_list_ic_data.enabled {
        let channel = mod_list_ic_data.channel_id;
        let mod_list_messages_arc = Arc::clone(&guild_config.read().await.mod_list_messages);
        if mod_list_messages_arc.read().await.is_empty() {
            let messages = channel
                .messages(ctx, |gm| gm.limit(MOD_LIST.len() as u64))
                .await?;
            channel.delete_messages(ctx, messages).await?;
            match scheduled_modlist(ctx, channel, &MOD_LIST).await {
                Err(why) => println!("Failed to update mod list (send messages step) in guild {guild}, channel {channel} due to a following error: {why}"),
                Ok(message_ids) => {
                    {
                        let mut mod_list_messages = mod_list_messages_arc.write().await;
                        mod_list_messages.clear();
                        mod_list_messages.extend(message_ids);
                    };
                    guild_config.read().await.write().await?;
                }
            }
        } else {
            edit_update_mod_list(ctx, channel, guild, mod_list_messages_arc).await?;
        }
    }
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
        e.field(
            "Download",
            format!(
                "[From official mod portal]({})\n[From Factorio Launcher storage]({})",
                download.official, download.launcher
            ),
            true,
        );
    }
    if let Some(latest_version) = data.latest_version {
        e.field("Latest version", latest_version, true);
    }
    e.field(
        "Downloaded",
        format!("{} times", data.downloads_count),
        true,
    );
    e.field(
        "Author",
        format!("[{0}](https://mods.factorio.com/user/{0})", data.author),
        true,
    );
    if let Some(timestamp) = data.timestamp {
        e.timestamp(timestamp)
            .footer(|f| f.text("Latest version was released at:"));
    }
    if let Some(thumbnail_url) = data.thumbnail_url {
        e.thumbnail(thumbnail_url);
    }
    e
}

async fn get_mod_info(
    ctx: &Context,
    mod_name: &str,
) -> Result<ModData, Box<dyn Error + Send + Sync>> {
    let client_data = ctx.data.read().await;
    let reqwest_client = client_data.get::<FactorioReqwestClient>().unwrap();
    let api_response = reqwest_client
        .get(format!("{MODPORTAL_URL}/api/mods/{mod_name}"))
        .send()
        .await?;
    if api_response.status() == StatusCode::OK {
        parse_mod_data(api_response, mod_name).await
    } else {
        let new_mod_name = find_mod(ctx, mod_name).await;
        if let Ok(mname) = new_mod_name {
            let api_response =
                reqwest::get(format!("{}/api/mods/{}", MODPORTAL_URL, &mname)).await?;
            if api_response.status() == StatusCode::OK {
                return parse_mod_data(api_response, &mname).await;
            }
        }
        Err(ModError::NotFound.into())
    }
}

async fn parse_mod_data(
    api_response: reqwest::Response,
    mod_name: &str,
) -> Result<ModData, Box<dyn Error + Send + Sync>> {
    let mut mod_info: ModInfo = api_response.json().await?;
    mod_info
        .releases
        .sort_by(|rls1, rls2| rls1.version.cmp(&rls2.version));
    let latest_release = mod_info.releases.last();
    let mut result = ModData::new(
        mod_info.title,
        mod_info.summary,
        format!("{MODPORTAL_URL}/mod/{mod_name}"),
        SUCCESS_EMBED_COLOR,
        mod_info.downloads_count,
        mod_info.owner,
    );
    if let Some(lrls) = latest_release {
        result.timestamp = Some(lrls.released_at.clone().into());
        result.game_version = Some(lrls.info_json.factorio_version.clone());
        result.download = Some(ModDownload {
            official: format!("{}{}", MODPORTAL_URL, lrls.download_url),
            launcher: format!("{}/{}/{}.zip", LAUNCHER_URL, mod_name, lrls.version),
        });
        result.latest_version = Some(lrls.version.to_string());
    }
    if mod_info.thumbnail != "/assets/.thumb.png" {
        result.thumbnail_url =
            format!("https://mods-data.factorio.com{}", mod_info.thumbnail).into();
    }
    Ok(result)
}

async fn find_mod(ctx: &Context, mod_name: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
    let client_data = ctx.data.read().await;
    let reqwest_client = client_data.get::<FactorioReqwestClient>().unwrap();
    let search_response = reqwest_client
        .get(format!("{MODPORTAL_URL}/query/{mod_name}"))
        .send()
        .await?;
    if search_response.status() == StatusCode::OK {
        let selector = Selector::parse("h2.mb0").unwrap();
        let document = Html::parse_document(&search_response.text().await?);
        match document.select(&selector).next() {
            Some(elem) => {
                let asel = Selector::parse("a").unwrap();
                match elem.select(&asel).next() {
                    Some(mod_link) => match mod_link.value().attr("href") {
                        Some(link) => Ok(String::from(&link[5..])),
                        None => Err(ModError::NotFound.into()),
                    },
                    None => Err(ModError::NotFound.into()),
                }
            }
            None => Err(ModError::NotFound.into()),
        }
    } else {
        Err(ModError::NotFound.into())
    }
}

#[group]
#[commands(mods_statistics, modlist)]
#[description("Commands related to Factorio mods")]
#[summary("Factorio mods")]
struct Factorio;

#[derive(Error, Debug)]
pub enum ModError {
    #[error("Mod not found")]
    NotFound,
}
