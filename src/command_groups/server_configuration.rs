use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::{CommandResult, Args};
use crate::guild_config::{GuildConfigManagerKey, CommandDisability};

/// Enables specified command and disables all filtering
#[command("enable")]
#[usage("<command name>")]
async fn enable_command(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let guild = msg.guild_id.unwrap().to_guild_cached(ctx).await.unwrap();
    let guild_config_lock = data.get::<GuildConfigManagerKey>().unwrap().get_guild_config(&guild).await?;
    let mut guild_config = guild_config_lock.get().write().await;
    let command_name = args.quoted().trimmed().single::<String>()?;
    guild_config.edit_command_filter(&command_name, |e| {
        e.enable()
    }).await?;
    msg.channel_id.say(&ctx.http, format!("Enabled command `{}`", command_name)).await?;
    Ok(())
}

/// Disables specified command for the entire guild
#[command("disable")]
#[usage("<command name>")]
async fn disable_command(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let guild = msg.guild_id.unwrap().to_guild_cached(ctx).await.unwrap();
    let guild_config_lock = data.get::<GuildConfigManagerKey>().unwrap().get_guild_config(&guild).await?;
    let mut guild_config = guild_config_lock.get().write().await;
    let command_name = args.quoted().trimmed().single::<String>()?;
    guild_config.edit_command_filter(&command_name, |e| {
        e.disable()
    }).await?;
    msg.channel_id.say(&ctx.http, format!("Disabled command `{}`", command_name)).await?;
    Ok(())
}

/// Sets filteing to whitelist and sets the filter list
#[command("whitelist")]
#[usage("<command name> [, channel, channel...]")]
async fn whitelist_command(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    args.trimmed().quoted();
    let data = ctx.data.read().await;
    let guild = msg.guild_id.unwrap().to_guild_cached(ctx).await.unwrap();
    let guild_config_lock = data.get::<GuildConfigManagerKey>().unwrap().get_guild_config(&guild).await?;
    let mut guild_config = guild_config_lock.get().write().await;
    let command_name = args.single::<String>()?;
    let mut arg_iter = args.iter::<ChannelId>();
    let filter_list = arg_iter.quoted().trimmed();
    let mut channels = Vec::with_capacity(filter_list.size_hint().0);
    for channel in filter_list {
        channels.push(channel?);
    }
    let channel_cnt = channels.len();
    guild_config.edit_command_filter(&command_name, |e| {
        e.filter_type(CommandDisability::Whitelisted).channels(channels)
    }).await?;
    msg.channel_id.say(&ctx.http, format!("Set filtering for command `{}` to whitelist. {} channels total", command_name, channel_cnt)).await?;
    Ok(())
}

/// Sets filteing to whitelist and sets the filter list
#[command("blacklist")]
#[usage("<command name> [, channel, channel...]")]
async fn blacklist_command(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    args.trimmed().quoted();
    let data = ctx.data.read().await;
    let guild = msg.guild_id.unwrap().to_guild_cached(ctx).await.unwrap();
    let guild_config_lock = data.get::<GuildConfigManagerKey>().unwrap().get_guild_config(&guild).await?;
    let mut guild_config = guild_config_lock.get().write().await;
    let command_name = args.single::<String>()?;
    let mut arg_iter = args.iter::<ChannelId>();
    let filter_list = arg_iter.quoted().trimmed();
    let mut channels = Vec::with_capacity(filter_list.size_hint().0);
    for channel in filter_list {
        channels.push(channel?);
    }
    let channel_cnt = channels.len();
    guild_config.edit_command_filter(&command_name, |e| {
        e.filter_type(CommandDisability::Blacklisted).channels(channels)
    }).await?;
    msg.channel_id.say(&ctx.http, format!("Set filtering for command `{}` to blacklist. {} channels total", command_name, channel_cnt)).await?;
    Ok(())
}

#[group]
#[prefix("command")]
#[only_in(guilds)]
#[commands(enable_command, disable_command, whitelist_command, blacklist_command)]
struct ConfigCommands;

#[group]
#[required_permissions(ADMINISTRATOR)]
#[sub_groups(ConfigCommands)]
#[only_in(guilds)]
struct ServerConfiguration;
