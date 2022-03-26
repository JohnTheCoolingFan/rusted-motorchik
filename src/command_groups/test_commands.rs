use serenity::client::Context;
use serenity::model::channel::Message;
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::{CommandResult, Args};
use serenity::utils::content_safe;
use crate::content_safe_settings;

/// Simply return the text that was passed to this command
#[command]
async fn test(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let settings = content_safe_settings(msg);
    let content = content_safe(&ctx.cache, &args.rest(), &settings).await;
    msg.channel_id.say(&ctx.http, &content).await?;
    Ok(())
}

/// Count the amount of arguments and nicely print them
#[command]
#[aliases(advtest, atest)]
async fn advanced_test(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let settings = content_safe_settings(msg);
    let arg_cnt = args.len();
    let content = content_safe(&ctx.cache, {
        let mut joined = args
        .iter::<String>()
        .quoted()
        .trimmed()
        .filter_map(|s| match s {
            Ok(rs) => Some(rs),
            Err(_) => None
        })
        .fold(String::new(), |s1, s2| s1 + &s2 + ", ");
        joined.pop();
        joined.pop();
        joined
    }, &settings).await;
    msg.channel_id.say(&ctx.http, format!("Passed {} arguments: {}", arg_cnt, content)).await?;
    Ok(())
}

#[group]
#[commands(test, advanced_test)]
struct TestCommands;
