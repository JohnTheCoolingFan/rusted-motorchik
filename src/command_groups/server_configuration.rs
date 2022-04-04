use std::str::FromStr;
use std::error::Error;
use thiserror::Error;
use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::{CommandResult, Args, ArgError};
use crate::guild_config::{GuildConfigManager, CommandDisability, InfoChannelType};

/// Enable (globally for the entire server/guild) specified command
#[command("enable")]
#[num_args(1)]
#[usage("<command name>")]
#[example("rickroll")]
async fn enable_command(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let guild_config_arc = GuildConfigManager::get_guild_config_from_ctx(ctx, msg.guild_id.unwrap()).await?;
    let mut guild_config = guild_config_arc.write().await;
    let command_name = args.quoted().trimmed().single::<String>()?;
    guild_config.edit_command_filter(&command_name, |e| {
        e.enable()
    }).await?;
    msg.channel_id.say(&ctx.http, format!("Enabled command `{}`", command_name)).await?;
    Ok(())
}

/// Disable (globally for the entire server/guild) specified command
#[command("disable")]
#[num_args(1)]
#[usage("<command name>")]
#[example("rickroll")]
async fn disable_command(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let guild_config_arc = GuildConfigManager::get_guild_config_from_ctx(ctx, msg.guild_id.unwrap()).await?;
    let mut guild_config = guild_config_arc.write().await;
    let command_name = args.quoted().trimmed().single::<String>()?;
    guild_config.edit_command_filter(&command_name, |e| {
        e.disable()
    }).await?;
    msg.channel_id.say(&ctx.http, format!("Disabled command `{}`", command_name)).await?;
    Ok(())
}

/// Set what channels this command is allowed to run in
#[command("whitelist")]
#[min_args(1)]
#[usage("<command name> [, channel, channel...]")]
#[example("ping #ping-pong")]
async fn whitelist_command(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    args.trimmed().quoted();
    let command_name = args.single::<String>()?;
    let channels: Vec<ChannelId> = args.iter::<ChannelId>().quoted().trimmed().collect::<Result<Vec<ChannelId>, ArgError<<ChannelId as FromStr>::Err>>>()?;
    let mentions = channels.iter().map(|c| c.mention().to_string()).collect::<Vec<String>>().join("\n");
    let guild_config_arc = GuildConfigManager::get_guild_config_from_ctx(ctx, msg.guild_id.unwrap()).await?;
    let mut guild_config = guild_config_arc.write().await;
    guild_config.edit_command_filter(&command_name, |e| {
        e.filter_type(CommandDisability::Whitelisted).channels(channels)
    }).await?;
    msg.channel_id.say(&ctx.http, format!("Set filtering for command `{}` to whitelist:\n{}", command_name, mentions)).await?;
    Ok(())
}

/// Set what channels this command is not allowed to run in
#[command("blacklist")]
#[min_args(1)]
#[usage("<command name> [, channel, channel...]")]
#[example("rickroll #serious-talk")]
async fn blacklist_command(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    args.trimmed().quoted();
    let command_name = args.single::<String>()?;
    let channels = args.iter().quoted().trimmed().collect::<Result<Vec<ChannelId>, ArgError<<ChannelId as FromStr>::Err>>>()?;
    let mentions = channels.iter().map(|c| c.mention().to_string()).collect::<Vec<String>>().join("\n");
    let guild_config_arc = GuildConfigManager::get_guild_config_from_ctx(ctx, msg.guild_id.unwrap()).await?;
    let mut guild_config = guild_config_arc.write().await;
    guild_config.edit_command_filter(&command_name, |e| {
        e.filter_type(CommandDisability::Blacklisted).channels(channels)
    }).await?;
    msg.channel_id.say(&ctx.http, format!("Set filtering for command `{}` to blacklist:\n{}", command_name, mentions)).await?;
    Ok(())
}

/// Commands for specifiyng where and which command can be executed.
#[group("Configuration")]
#[prefix("command")]
#[only_in(guilds)]
#[commands(enable_command, disable_command, whitelist_command, blacklist_command)]
#[summary("Command conditions")]
struct ConfigCommands;

/// Enable specified info channel. Recommended to set a channel first.
#[command("enable")]
#[num_args(1)]
#[usage("(welcome,log,mod-list)")]
#[example("welcome")]
async fn enable_ic(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    args.trimmed().quoted();
    let ic_type = args.single::<InfoChannelType>()?;
    let guild_config_arc = GuildConfigManager::get_guild_config_from_ctx(ctx, msg.guild_id.unwrap()).await?;
    let mut guild_config = guild_config_arc.write().await;
    guild_config.edit(|e| {
        e.info_channel(ic_type, |i| {
            i.state(true)
        })
    }).await?;
    msg.channel_id.say(&ctx.http, format!("Successfully enabled `{}` info channel.", ic_type.as_ref())).await?;
    Ok(())
}

/// Disable specified info channel.
#[command("disable")]
#[num_args(1)]
#[usage("(welcome,log,mod-list)")]
#[example("log")]
async fn disable_ic(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    args.trimmed().quoted();
    let ic_type = args.single::<InfoChannelType>()?;
    let guild_config_arc = GuildConfigManager::get_guild_config_from_ctx(ctx, msg.guild_id.unwrap()).await?;
    let mut guild_config = guild_config_arc.write().await;
    guild_config.edit(|e| {
        e.info_channel(ic_type, |i| {
            i.state(false)
        })
    }).await?;
    msg.channel_id.say(&ctx.http, format!("Successfully disabled `{}` info channel.", ic_type.as_ref())).await?;
    Ok(())
}

/// Set channel for info channel.
#[command("set")]
#[num_args(2)]
#[usage("(welcome,log,mod-list), <channel>")]
#[example("welcome, #welcome")]
async fn set_ic(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    args.trimmed().quoted();
    let ic_type = args.single::<InfoChannelType>()?;
    let channel = args.single::<ChannelId>()?;
    let guild_config_arc = GuildConfigManager::get_guild_config_from_ctx(ctx, msg.guild_id.unwrap()).await?;
    let mut guild_config = guild_config_arc.write().await;
    guild_config.edit(|e| {
        e.info_channel(ic_type, |i| {
            i.channel(channel)
        })
    }).await?;
    msg.channel_id.say(&ctx.http, format!("Channel for `{}` is set to {}", ic_type.as_ref(), channel.mention())).await?;
    Ok(())
}

/// Control Info Channels
#[group("Info Channels")]
#[prefix("info_channel")]
#[only_in(guilds)]
#[commands(enable_ic, disable_ic, set_ic)]
struct InfoChannelsCommands;

/// Set default roles that are given to newcomers. Waits until server moderation requirements are met and member has read the rules.
/// If no roles are specified, the list will be cleared.
#[command]
#[usage("[role, role...]")]
#[example("@Newbie")]
#[example("@Newcomer, @JustJoined")]
async fn default_roles(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    args.trimmed().quoted();
    let roles = args.iter().quoted().trimmed().collect::<Result<Vec<RoleId>, ArgError<<RoleId as FromStr>::Err>>>()?;
    let roles_cnt = roles.len();
    let guild_config_arc = GuildConfigManager::get_guild_config_from_ctx(ctx, msg.guild_id.unwrap()).await?;
    let mut guild_config = guild_config_arc.write().await;
    guild_config.edit(|e| {
        e.default_roles(roles)
    }).await?;
    msg.channel_id.say(&ctx.http, format!("Default roles ({}) set", roles_cnt)).await?;
    Ok(())
}

/// Enable or disable linked message lookup. If enabled, bot will scan for discord message links in
/// messages and send contents of each linked message in reply.
#[command]
#[usage("(enable|disable)")]
#[example("disable")]
async fn message_link_lookup(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let term = args.trimmed().quoted().single::<String>()?;
    let guild_config = GuildConfigManager::get_guild_config_from_ctx(ctx, msg.guild_id.unwrap()).await?;
    if term == "enable" {
        guild_config.write().await.message_link_lookup = true;
    } else if term == "disable" {
        guild_config.write().await.message_link_lookup = false;
    } else {
        return Err(ServerConfigurationError::InvalidActionTerm(term).into())
    }
    Ok(())
}

/// Guild-specific bot settings
#[group("Server Configuration")]
#[required_permissions(ADMINISTRATOR)]
#[sub_groups(ConfigCommands, InfoChannelsCommands)]
#[only_in(guilds)]
#[commands(default_roles, message_link_lookup)]
struct ServerConfiguration;

#[derive(Debug, Error)]
pub enum ServerConfigurationError {
    #[error("Invalid action term: `{0}`")]
    InvalidActionTerm(String)
}
