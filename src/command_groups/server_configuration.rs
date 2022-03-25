use std::str::FromStr;
use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::{CommandResult, Args, ArgError};
use crate::guild_config::{GuildConfigManager, CommandDisability, InfoChannelType};

/// Enables specified command and disables all filtering
#[command("enable")]
#[usage("<command name>")]
async fn enable_command(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let guild = msg.guild_id.unwrap().to_guild_cached(ctx).await.unwrap();
    let gc_manager = {
        let data = ctx.data.read().await;
        data.get::<GuildConfigManager>().unwrap().clone()
    };
    let guild_config_arc = gc_manager.get_guild_config(&guild).await?;
    let mut guild_config = guild_config_arc.write().await;
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
    let guild = msg.guild_id.unwrap().to_guild_cached(ctx).await.unwrap();
    let gc_manager = {
        let data = ctx.data.read().await;
        data.get::<GuildConfigManager>().unwrap().clone()
    };
    let guild_config_arc = gc_manager.get_guild_config(&guild).await?;
    let mut guild_config = guild_config_arc.write().await;
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
    let command_name = args.single::<String>()?;
    let channels: Vec<ChannelId> = args.iter::<ChannelId>().quoted().trimmed().collect::<Result<Vec<ChannelId>, ArgError<<ChannelId as FromStr>::Err>>>()?;
    let mentions = channels.iter().map(|c| c.mention().to_string()).collect::<Vec<String>>().join("\n");
    let guild = msg.guild_id.unwrap().to_guild_cached(ctx).await.unwrap();
    let gc_manager = {
        let data = ctx.data.read().await;
        data.get::<GuildConfigManager>().unwrap().clone()
    };
    let guild_config_arc = gc_manager.get_guild_config(&guild).await?;
    let mut guild_config = guild_config_arc.write().await;
    guild_config.edit_command_filter(&command_name, |e| {
        e.filter_type(CommandDisability::Whitelisted).channels(channels)
    }).await?;
    msg.channel_id.say(&ctx.http, format!("Set filtering for command `{}` to whitelist:\n{}", command_name, mentions)).await?;
    Ok(())
}

/// Sets filteing to whitelist and sets the filter list
#[command("blacklist")]
#[usage("<command name> [, channel, channel...]")]
async fn blacklist_command(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    args.trimmed().quoted();
    let command_name = args.single::<String>()?;
    let channels = args.iter().quoted().trimmed().collect::<Result<Vec<ChannelId>, ArgError<<ChannelId as FromStr>::Err>>>()?;
    let mentions = channels.iter().map(|c| c.mention().to_string()).collect::<Vec<String>>().join("\n");
    let guild = msg.guild_id.unwrap().to_guild_cached(ctx).await.unwrap();
    let gc_manager = {
        let data = ctx.data.read().await;
        data.get::<GuildConfigManager>().unwrap().clone()
    };
    let guild_config_arc = gc_manager.get_guild_config(&guild).await?;
    let mut guild_config = guild_config_arc.write().await;
    guild_config.edit_command_filter(&command_name, |e| {
        e.filter_type(CommandDisability::Blacklisted).channels(channels)
    }).await?;
    msg.channel_id.say(&ctx.http, format!("Set filtering for command `{}` to blacklist:\n{}", command_name, mentions)).await?;
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
    let guild = msg.guild_id.unwrap().to_guild_cached(ctx).await.unwrap();
    let gc_manager = {
        let data = ctx.data.read().await;
        data.get::<GuildConfigManager>().unwrap().clone()
    };
    let guild_config_arc = gc_manager.get_guild_config(&guild).await?;
    let mut guild_config = guild_config_arc.write().await;
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
    let guild = msg.guild_id.unwrap().to_guild_cached(ctx).await.unwrap();
    let gc_manager = {
        let data = ctx.data.read().await;
        data.get::<GuildConfigManager>().unwrap().clone()
    };
    let guild_config_arc = gc_manager.get_guild_config(&guild).await?;
    let mut guild_config = guild_config_arc.write().await;
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
    let guild = msg.guild_id.unwrap().to_guild_cached(ctx).await.unwrap();
    let gc_manager = {
        let data = ctx.data.read().await;
        data.get::<GuildConfigManager>().unwrap().clone()
    };
    let guild_config_arc = gc_manager.get_guild_config(&guild).await?;
    let mut guild_config = guild_config_arc.write().await;
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

/// Set default roles
#[command]
#[usage("[role, role...]")]
async fn default_roles(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    args.trimmed().quoted();
    let roles = args.iter().quoted().trimmed().collect::<Result<Vec<RoleId>, ArgError<<RoleId as FromStr>::Err>>>()?;
    let roles_cnt = roles.len();
    let guild = msg.guild_id.unwrap().to_guild_cached(ctx).await.unwrap();
    let gc_manager = {
        let data = ctx.data.read().await;
        data.get::<GuildConfigManager>().unwrap().clone()
    };
    let guild_config_arc = gc_manager.get_guild_config(&guild).await?;
    let mut guild_config = guild_config_arc.write().await;
    guild_config.edit(|e| {
        e.default_roles(roles)
    }).await?;
    msg.channel_id.say(&ctx.http, format!("Default roles ({}) set", roles_cnt)).await?;
    Ok(())
}

/// Guild-specific bot settings
#[group]
#[required_permissions(ADMINISTRATOR)]
#[sub_groups(ConfigCommands, InfoChannelsCommands)]
#[only_in(guilds)]
#[commands(default_roles)]
struct ServerConfiguration;
