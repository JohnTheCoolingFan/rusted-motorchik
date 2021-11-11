use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::{CommandResult, Args};
use crate::guild_config::{GuildConfigManagerKey, CommandDisability, InfoChannelType};

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

/// Enables specified info channel
#[command("enable")]
#[usage("(welcome,log,mod-list)")]
async fn enable_ic(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    args.trimmed().quoted();
    let ic_type = args.single::<InfoChannelType>()?;
    let data = ctx.data.read().await;
    let gc_manager = data.get::<GuildConfigManagerKey>().unwrap();
    let guild = msg.guild_id.unwrap().to_guild_cached(ctx).await.unwrap();
    let guild_config_lock = gc_manager.get_guild_config(&guild).await?;
    let mut guild_config = guild_config_lock.get().write().await;
    guild_config.edit(|e| {
        e.info_channel(ic_type, |i| {
            i.state(true)
        })
    }).await?;
    msg.channel_id.say(&ctx.http, format!("Successfully enabled `{}` info channel.", ic_type.as_ref())).await?;
    Ok(())
}

/// Enables specified info channel
#[command("disable")]
#[usage("(welcome,log,mod-list)")]
async fn disable_ic(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    args.trimmed().quoted();
    let ic_type = args.single::<InfoChannelType>()?;
    let data = ctx.data.read().await;
    let gc_manager = data.get::<GuildConfigManagerKey>().unwrap();
    let guild = msg.guild_id.unwrap().to_guild_cached(ctx).await.unwrap();
    let guild_config_lock = gc_manager.get_guild_config(&guild).await?;
    let mut guild_config = guild_config_lock.get().write().await;
    guild_config.edit(|e| {
        e.info_channel(ic_type, |i| {
            i.state(false)
        })
    }).await?;
    msg.channel_id.say(&ctx.http, format!("Successfully disabled `{}` info channel.", ic_type.as_ref())).await?;
    Ok(())
}

/// Set channel for Info Channel
#[command("set")]
#[usage("(welcome,log,mod-list), [channel]")]
async fn set_ic(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    args.trimmed().quoted();
    let ic_type = args.single::<InfoChannelType>()?;
    let channel = args.single::<ChannelId>()?;
    let data = ctx.data.read().await;
    let gc_manager = data.get::<GuildConfigManagerKey>().unwrap();
    let guild = msg.guild_id.unwrap().to_guild_cached(ctx).await.unwrap();
    let guild_config_lock = gc_manager.get_guild_config(&guild).await?;
    let mut guild_config = guild_config_lock.get().write().await;
    guild_config.edit(|e| {
        e.info_channel(ic_type, |i| {
            i.channel(channel)
        })
    }).await?;
    msg.channel_id.say(&ctx.http, format!("Channel for `{}` is set to {}", ic_type.as_ref(), channel.mention())).await?;
    Ok(())
}

/// Control Info Channels settings
#[group]
#[prefix("info_channel")]
#[only_in(guilds)]
#[commands(enable_ic, disable_ic, set_ic)]
struct InfoChannelsCommands;

/// Guild-specific bot settings
#[group]
#[required_permissions(ADMINISTRATOR)]
#[sub_groups(ConfigCommands, InfoChannelsCommands)]
#[only_in(guilds)]
struct ServerConfiguration;
