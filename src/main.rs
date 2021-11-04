use std::env;
use std::collections::HashSet;
use serenity::client::bridge::gateway::GatewayIntents;
use serenity::async_trait;
use serenity::client::{Client, Context, EventHandler};
use serenity::model::{channel::Message, gateway::Ready};
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::{StandardFramework, CommandResult, Args};
use serenity::utils::{content_safe, ContentSafeOptions};
use serenity::http::Http;

pub fn content_safe_settings(msg: &Message) -> ContentSafeOptions {
    match &msg.guild_id {
        Some(guild_id) => ContentSafeOptions::default().clean_channel(false).display_as_member_from(guild_id),
        _ => ContentSafeOptions::default().clean_channel(false).clean_role(false)
    }
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("Connected as {}", ready.user.name);
    }
}

#[command]
async fn test(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let settings = content_safe_settings(msg);
    let content = content_safe(&ctx.cache, &args.rest(), &settings).await;
    msg.channel_id.say(&ctx.http, &content).await?;
    Ok(())
}

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

#[tokio::main]
async fn main() {
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let http = Http::new_with_token(&token);

    let (owners, bot_id) = match http.get_current_application_info().await {
        Ok(info) => {
            let mut owners = HashSet::new();
            match info.team {
                Some(team) => owners.insert(team.owner_user_id),
                _ => owners.insert(info.owner.id)
            };
            match http.get_current_user().await {
                Ok(bot_id) => (owners, bot_id.id),
                Err(why) => panic!("Could not access the bot id: {:?}", why)
            }
        }
        Err(why) => panic!("could not access application info: {:?}", why)
    };

    let framework = StandardFramework::new()
        .configure(|c| c
            .with_whitespace(true)
            .on_mention(Some(bot_id))
            .prefix("$!")
            .delimiters(vec![" ", ", ", ","])
            .owners(owners))
        .group(&TESTCOMMANDS_GROUP);

    let mut client = Client::builder(&token)
        .event_handler(Handler)
        .framework(framework)
        .intents(GatewayIntents::all())
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
