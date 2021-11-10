use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::{CommandResult, Args};
use crate::guild_config::{GuildConfigManagerKey, CommandDisability};

/// Enables specified command and disables all filtering
#[command("enable")]
async fn enable_command(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let guild = msg.guild_id.unwrap().to_guild_cached(ctx).await.unwrap();
    let guild_config_lock = data.get::<GuildConfigManagerKey>().unwrap().get_guild_config(&guild).await?;
    let mut guild_config = guild_config_lock.get().write().await;
    guild_config.edit_command_filter(&args.quoted().trimmed().single::<String>()?, |e| {
        e.enable()
    }).await
}

/// Disables specified command for the entire guild
#[command("disable")]
async fn disable_command(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let guild = msg.guild_id.unwrap().to_guild_cached(ctx).await.unwrap();
    let guild_config_lock = data.get::<GuildConfigManagerKey>().unwrap().get_guild_config(&guild).await?;
    let mut guild_config = guild_config_lock.get().write().await;
    guild_config.edit_command_filter(&args.quoted().trimmed().single::<String>()?, |e| {
        e.disable()
    }).await
}

/// Sets filteing to whitelist and sets the filter list
#[command("whitelist")]
async fn whitelist_command(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    args.trimmed().quoted();
    let data = ctx.data.read().await;
    let guild = msg.guild_id.unwrap().to_guild_cached(ctx).await.unwrap();
    let guild_config_lock = data.get::<GuildConfigManagerKey>().unwrap().get_guild_config(&guild).await?;
    let mut guild_config = guild_config_lock.get().write().await;
    let command_name = args.single::<String>()?;
    let arg_channels_iter = args.iter::<ChannelId>();
    let mut filter_list: Vec<ChannelId> = Vec::with_capacity(arg_channels_iter.size_hint().0);
    for channel in arg_channels_iter {
        filter_list.push(channel?);
    }
    guild_config.edit_command_filter(&command_name, |e| {
        e.filter_type(CommandDisability::Whitelisted).channels(filter_list)
    }).await
}

/// Sets filteing to whitelist and sets the filter list
#[command("blacklist")]
async fn blacklist_command(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    args.trimmed().quoted();
    let data = ctx.data.read().await;
    let guild = msg.guild_id.unwrap().to_guild_cached(ctx).await.unwrap();
    let guild_config_lock = data.get::<GuildConfigManagerKey>().unwrap().get_guild_config(&guild).await?;
    let mut guild_config = guild_config_lock.get().write().await;
    let command_name = args.single::<String>()?;
    let arg_channels_iter = args.iter::<ChannelId>();
    let mut filter_list: Vec<ChannelId> = Vec::with_capacity(arg_channels_iter.size_hint().0);
    for channel in arg_channels_iter {
        filter_list.push(channel?);
    }
    guild_config.edit_command_filter(&command_name, |e| {
        e.filter_type(CommandDisability::Blacklisted).channels(filter_list)
    }).await
}

#[group]
#[prefix("command")]
#[commands(enable_command, disable_command, whitelist_command, blacklist_command)]
struct ConfigCommands;

#[group]
#[required_permissions(ADMINISTRATOR)]
#[sub_groups(ConfigCommands)]
struct ServerConfiguration;
