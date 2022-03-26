use serenity::client::Context;
use serenity::model::id::{ChannelId, UserId};
use serenity::model::channel::Message;
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::{CommandResult, Args};

#[command]
async fn say(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    msg.delete(&ctx.http).await?;
    args.single::<ChannelId>()?.say(&ctx.http, args.trimmed().quoted().single::<String>()?).await?;
    Ok(())
}

#[command]
async fn say_dm(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    msg.delete(&ctx.http).await?;
    let dm_channel = args.single::<UserId>()?.create_dm_channel(&ctx.http).await?;
    dm_channel.say(&ctx.http, args.trimmed().quoted().single::<String>()?).await?;
    Ok(())
}

/// Nothing for you
#[group]
#[owners_only]
#[commands(say, say_dm)]
struct ServiceTools;
