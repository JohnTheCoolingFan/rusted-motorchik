use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::{CommandResult, Args};
use std::time::Duration;
use crate::{Handler, BanMessageIgnoreList};
use std::sync::Arc;

const CLEARCHAT_WAIT_DURATION: Duration = Duration::from_secs(3);

/// Removes X last messages from the channel this command is invoked in
#[command]
#[aliases(clear, cl)]
#[required_permissions(MANAGE_MESSAGES)]
#[num_args(1)]
#[usage("X")]
#[example("10")]
async fn clearchat(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let count = args.single::<u64>()?;
    msg.delete(&ctx.http).await?;
    let messages = msg.channel_id.messages(&ctx.http, |gm| gm.limit(count)).await?;
    let deleted_count = messages.len();
    msg.channel_id.delete_messages(&ctx.http, messages).await?;
    let confirmation_msg = msg.channel_id.say(&ctx.http, format!("Deleted {} message(s)", deleted_count)).await?;
    tokio::time::sleep(CLEARCHAT_WAIT_DURATION).await;
    confirmation_msg.delete(&ctx.http).await?;
    Ok(())
}

/// Kicks specified member and logs to a log channel if it is enabled
#[command]
#[required_permissions(KICK_MEMBERS)]
#[num_args(1)]
#[usage("@Member")]
#[example("@Wumpus")]
async fn kick(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let user_id = args.single::<UserId>()?;
    let reason = match args.is_empty() {
        true => None,
        false => Some(args.single::<String>()?)
    };
    let user = user_id.to_user(ctx).await?;
    msg.guild_id.unwrap().kick(ctx, user_id).await?;
    Handler::log_channel_kick_message(ctx, msg.guild_id.unwrap(), &user, &msg.author, reason).await;
    Ok(())
}

/// "Bans specified member and logs to a log channel if it is enabled
#[command]
#[required_permissions(BAN_MEMBERS)]
#[num_args(1)]
#[usage("@Member")]
#[example("@VeryAgressiveSpammer")]
async fn ban(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let user = args.single::<UserId>()?;
    let reason = match args.is_empty() {
        true => None,
        false => Some(args.single::<String>()?)
    };
    let ignore_list = Arc::clone(ctx.data.read().await.get::<BanMessageIgnoreList>().unwrap());
    ignore_list.write().await.insert((msg.guild_id.unwrap(), user));
    msg.guild_id.unwrap().ban(ctx, &user, 0).await?;
    Handler::log_channel_ban_message(ctx, msg.guild_id.unwrap(), &user.to_user(ctx).await?, Some(&msg.author), reason).await;
    Ok(())
}

/// Useful tools for moderation
#[group]
#[commands(clearchat, kick, ban)]
struct Moderation;
