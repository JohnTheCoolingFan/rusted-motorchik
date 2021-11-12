mod command_groups;
mod guild_config;

use command_groups::*;

use std::env;
use std::collections::HashSet;
use serenity::model::prelude::*;
use serenity::client::bridge::gateway::GatewayIntents;
use serenity::async_trait;
use serenity::client::{Client, Context, EventHandler};
use serenity::model::{channel::Message, gateway::Ready};
use serenity::framework::standard::{StandardFramework, CommandResult, Args, CommandGroup, HelpOptions};
use serenity::framework::standard::{macros::{help, hook}, help_commands};
use serenity::utils::ContentSafeOptions;
use serenity::http::Http;
use guild_config::GuildConfigManager;

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

    async fn message(&self, ctx: Context, msg: Message) {
        if msg.is_private() && !msg.is_own(&ctx.cache).await {
            if let Ok(appinfo) = ctx.http.get_current_application_info().await {
                let owner = appinfo.owner;
                if let Err(why) = owner.dm(&ctx.http, |m| m.content(format!("I have received a message from {}:\n{}", msg.author.tag(), msg.content))).await {
                    println!("Failed to redirect message: {}", why)
                }
            }
        }
    }
}

#[help]
#[command_not_found_text = "Could not find command: {}"]
#[max_levenshtein_distance(3)]
#[lacking_role = "hide"]
#[lacking_ownership = "hide"]
#[lacking_permissions = "hide"]
#[lacking_conditions = "strike"]
async fn my_help(ctx: &Context, msg: &Message, args: Args, hopt: &'static HelpOptions, groups: &[&'static CommandGroup], owners: HashSet<UserId>) -> CommandResult {
    let _ = help_commands::with_embeds(ctx, msg, args, hopt, groups, owners).await;
    Ok(())
}

// TODO: hook before to check command filters

// TODO: send the error in a message and print if sending in message failed
#[hook]
async fn after(ctx: &Context, msg: &Message, command_name: &str, command_result: CommandResult) {
    if let Err(why) = command_result {
        println!("Command '{}' returned error {}", command_name, why);
        if let Err(why_echo) = msg.channel_id.send_message(&ctx.http, |m| {
            m.add_embed(|e| {
                e.color((255, 15, 15))
                    .title("Error duting running a command")
                    .description(format!("Error occured while running command `{}`:\n{}", command_name, why))
                    .footer(|f| {
                        f.text("Please contact bot author on github/gitlab/discord, see `source` command")
                    })
            })
        }).await {
            println!("Error sending command error report: {}", why_echo);
        }
    }
}

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
            .on_mention(Some(bot_id))
            .prefix("$!")
            .delimiters(vec![", ", ","])
            .owners(owners))
        .after(after)
        .help(&MY_HELP)
        .group(&TESTCOMMANDS_GROUP)
        .group(&FUNCOMMANDS_GROUP)
        .group(&MISCELLANEOUS_GROUP)
        .group(&FACTORIO_GROUP)
        .group(&MODERATION_GROUP)
        .group(&SERVICETOOLS_GROUP)
        .group(&SERVERCONFIGURATION_GROUP);

    let mut client = Client::builder(&token)
        .event_handler(Handler)
        .framework(framework)
        .intents(GatewayIntents::all())
        .await
        .expect("Err creating client");

    {
        let config_path = env::var("GUILD_CONFIG_HOME").expect("Expected GUILD_CONFIG_HOME path in the environment");
        let mut client_data = client.data.write().await;
        // TODO: get path from environment variable
        client_data.insert::<GuildConfigManager>(GuildConfigManager::new(&config_path));
        client_data.insert::<FactorioReqwestClient>(reqwest::Client::new());
    }

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
