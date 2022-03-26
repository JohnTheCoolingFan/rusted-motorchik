use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::{CommandResult, Args};
use std::time::Duration;
use crate::{Handler, BanMessageIgnoreList};
use std::sync::Arc;

#[command]
#[aliases(clear, cl)]
#[required_permissions(MANAGE_MESSAGES)]
async fn clearchat(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let count = args.single::<u64>()?;
    msg.delete(&ctx.http).await?;
    let messages = msg.channel_id.messages(&ctx.http, |gm| gm.limit(count)).await?;
    let deleted_count = messages.len();
    msg.channel_id.delete_messages(&ctx.http, messages).await?;
    let confirmation_msg = msg.channel_id.say(&ctx.http, format!("Deleted {} message(s)", deleted_count)).await?;
    tokio::time::sleep(Duration::from_secs(3)).await;
    confirmation_msg.delete(&ctx.http).await?;
    Ok(())
}

#[command]
#[required_permissions(KICK_MEMBERS)]
async fn kick(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let user = args.single::<UserId>()?;
    let reason = match args.is_empty() {
        true => None,
        false => Some(args.single::<String>()?)
    };
    Handler::log_channel_kick_message(ctx, msg.guild_id.unwrap(), &user, &msg.author, reason).await;
    Ok(())
}

#[command]
#[required_permissions(BAN_MEMBERS)]
async fn ban(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let user = args.single::<UserId>()?;
    let reason = match args.is_empty() {
        true => None,
        false => Some(args.single::<String>()?)
    };
    let ignore_list = Arc::clone(ctx.data.read().await.get::<BanMessageIgnoreList>().unwrap());
    ignore_list.write().await.insert((msg.guild_id.unwrap(), user));
    Handler::log_channel_ban_message(ctx, msg.guild_id.unwrap(), &user.to_user(ctx).await?, Some(&msg.author), reason).await;
    Ok(())
}

// Other commands require InfoChannels functionality to function as intended.

#[group]
#[commands(clearchat, kick)]
struct Moderation;
