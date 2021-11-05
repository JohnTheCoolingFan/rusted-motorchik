use serenity::client::Context;
use serenity::model::channel::Message;
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::{CommandResult, Args};
use tokio;
use std::time::Duration;

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

// Other commands require InfoChannels functionality to function as intended.

#[group]
#[commands(clearchat)]
struct Moderation;
