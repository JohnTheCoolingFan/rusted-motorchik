mod command_groups;
mod guild_config;
mod rolequeue;

use command_groups::*;

use std::time::Duration;
use thiserror::Error;
use std::error::Error;
use std::env;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use serenity::prelude::*;
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
use rolequeue::{RoleQueue, RoleQueueItem};

pub fn content_safe_settings(msg: &Message) -> ContentSafeOptions {
    match &msg.guild_id {
        Some(guild_id) => ContentSafeOptions::default().clean_channel(false).display_as_member_from(guild_id),
        _ => ContentSafeOptions::default().clean_channel(false).clean_role(false)
    }
}

struct Handler {
    is_loop_running: AtomicBool
}

async fn check_queued(ctx: Arc<Context>, item: &RoleQueueItem) -> Result<bool, Box<dyn Error + Send + Sync>> {
    let member = item.guild_id.member(&ctx.http, item.user_id).await?;
    let guild = item.guild_id.to_guild_cached(&ctx.cache).await;
    if let Some(guild) = guild {
        Ok(!(guild.verification_level >= VerificationLevel::Medium && (chrono::Utc::now() - member.user.created_at() > chrono::Duration::minutes(5)) || guild.verification_level >= VerificationLevel::High && (chrono::Utc::now() - member.joined_at.ok_or(QueueError::JoinDateMissing)? > chrono::Duration::minutes(10))))
    } else {
        Err(QueueError::GuildNotAvailable.into())
    }
}

#[derive(Debug, Error)]
enum QueueError {
    #[error("Guild is not available")]
    GuildNotAvailable,
    #[error("No join date")]
    JoinDateMissing
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("Connected as {}", ready.user.name);
    }

    async fn cache_ready(&self, ctx: Context, _guilds: Vec<GuildId>) {
        let ctx = Arc::new(ctx);

        if !self.is_loop_running.load(Ordering::Relaxed) {
            let ctx1 = Arc::clone(&ctx);
            tokio::spawn(async move {
                loop {
                    let queue = {
                        let data = ctx1.data.read().await;
                        Arc::clone(data.get::<RoleQueue>().unwrap())
                    };
                    let queue_clone = queue.read().await.clone();
                    let mut new_queue: Vec<RoleQueueItem> = Vec::with_capacity(queue_clone.len());
                    for item in queue_clone {
                        if let Ok(verdict) = check_queued(Arc::clone(&ctx1), &item).await {
                            if !verdict {
                                new_queue.push(item)
                            } else {
                                let member = item.guild_id.member(&ctx1.http, item.user_id).await;
                                if let Ok(member) = member {
                                    if let Err(why) = member.edit(&ctx1, |e| {
                                        e.roles(item.roles)
                                    }).await {
                                        println!("Failed to give roles to member {} in guild {}: {}", item.user_id, item.guild_id, why)
                                    }
                                }
                            }
                        }
                    }
                    tokio::time::sleep(Duration::from_secs(15)).await;
                }
            });
        }
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

    async fn guild_member_addition(&self, ctx: Context, guild_id: GuildId, new_member: Member) {
        let queue = {
            let data = ctx.data.read().await;
            Arc::clone(data.get::<RoleQueue>().unwrap())
        };
        let roles = {
            let guild = guild_id.to_guild_cached(&ctx.cache).await.unwrap();
            let gc_manager = {
                let data = ctx.data.read().await;
                data.get::<GuildConfigManager>().unwrap().clone()
            };
            let guild_config_arc = gc_manager.get_guild_config(&guild).await.unwrap();
            let guild_config = guild_config_arc.read().await;
            guild_config.default_roles().clone()
        };
        queue.write().await.push(RoleQueueItem{user_id: new_member.user.id, guild_id: new_member.guild_id, roles});
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
        .event_handler(Handler{
            is_loop_running: AtomicBool::new(false)
        })
        .framework(framework)
        .intents(GatewayIntents::all())
        .await
        .expect("Err creating client");

    {
        let config_path = env::var("GUILD_CONFIG_HOME").expect("Expected GUILD_CONFIG_HOME path in the environment");
        let mut client_data = client.data.write().await;
        client_data.insert::<RoleQueue>(Arc::new(RwLock::new(Vec::new())));
        client_data.insert::<GuildConfigManager>(Arc::new(GuildConfigManager::new(&config_path)));
        client_data.insert::<FactorioReqwestClient>(reqwest::Client::new());
    }

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
