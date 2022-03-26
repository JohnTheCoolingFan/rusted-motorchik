mod command_groups;
mod guild_config;
mod role_queue;

use command_groups::*;

use std::env;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
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
use guild_config::{GuildConfigManager, InfoChannelType};
use role_queue::RoleQueue;

const ROLE_QUEUE_INTERVAL: Duration = Duration::from_secs(30); // 30 seconds
const MOD_LIST_UPDATE_INTERVAL: Duration = Duration::from_secs(60 * 60); // 1 hour

pub fn content_safe_settings(msg: &Message) -> ContentSafeOptions {
    match &msg.guild_id {
        Some(guild_id) => ContentSafeOptions::default().clean_channel(false).display_as_member_from(guild_id),
        _ => ContentSafeOptions::default().clean_channel(false).clean_role(false)
    }
}

struct Handler {
    is_loop_running: AtomicBool
}

#[async_trait]
impl EventHandler for Handler {
    // Print account info
    async fn ready(&self, _: Context, ready: Ready) {
        println!("Connected as {}", ready.user.name);
    }

    // Redirect DMs to author
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

    async fn cache_ready(&self, ctx: Context, guilds: Vec<GuildId>) {
        println!("Cache built successfully!");

        let ctx = Arc::new(ctx);
        let guilds = Arc::new(guilds);

        if !self.is_loop_running.load(Ordering::Relaxed) {
            let ctx1 = Arc::clone(&ctx);

            // Members on hold for role giving
            tokio::spawn(async move {
                loop {
                    let queue = Arc::clone(ctx1.data.read().await.get::<RoleQueue>().unwrap());
                    let mut new_queue: Vec<(GuildId, UserId)> = Vec::with_capacity(queue.read().await.len());
                    for item in &*queue.read().await {
                        if let Ok(mut member) = item.0.member(Arc::clone(&ctx1.http), item.1).await {
                            if member.pending {
                                new_queue.push(*item)
                            } else {
                                let gc_manager = Arc::clone(ctx1.data.read().await.get::<GuildConfigManager>().unwrap());
                                let guild_cached = member.guild_id.to_guild_cached(&ctx1).await.unwrap();
                                if let Ok(guild_config) = gc_manager.get_guild_config(&guild_cached).await {
                                    if let Err(why) = member.add_roles(&ctx1, guild_config.read().await.default_roles()).await {
                                        println!("Failed to give roles to member {} of guild {}: {}", member.user.id, member.guild_id, why);
                                    }
                                }
                            }
                        }
                    };
                    let mut queue_write = queue.write().await;
                    queue_write.clear();
                    queue_write.append(&mut new_queue);
                    tokio::time::sleep(ROLE_QUEUE_INTERVAL).await;
                }
            });

            let ctx2 = Arc::clone(&ctx);
            let guilds1 = Arc::clone(&guilds);

            // Update mod list
            // TODO: somehow update list of guilds to iterate over (for example, bot added to a new
            //       guild while running)
            tokio::spawn(async move {
                loop {
                    let gc_manager = Arc::clone(ctx2.data.read().await.get::<GuildConfigManager>().unwrap());
                    for guild in &*guilds1 {
                        let guild_cached = guild.to_guild_cached(&ctx2).await.unwrap();
                        if let Ok(guild_config) = gc_manager.get_guild_config(&guild_cached).await {
                            if let Some(mod_list_ic_data) = guild_config.read().await.info_channels_data(InfoChannelType::ModList) {
                                if mod_list_ic_data.enabled {
                                    let channel = mod_list_ic_data.channel_id;
                                    if let Ok(messages) = channel.messages(&ctx2, |gm| gm.limit(MOD_LIST.len() as u64)).await {
                                        if let Err(why) = channel.delete_messages(&ctx2, messages).await {
                                            println!("Failed to update mod list at guild {} in channel {} due to a folloeing error: {}", guild, channel, why);
                                        }
                                    }
                                }
                            }
                        }
                    };

                    tokio::time::sleep(MOD_LIST_UPDATE_INTERVAL).await;
                }
            });
        }
    }

    // Member joined a guild
    async fn guild_member_addition(&self, ctx: Context, guild_id: GuildId, mut new_member: Member) {
        let queue = Arc::clone(ctx.data.read().await.get::<RoleQueue>().unwrap());
        if new_member.pending {
            let mut queue_write = queue.write().await;
            queue_write.push((guild_id, new_member.user.id))
        } else {
            let gc_manager = Arc::clone(ctx.data.read().await.get::<GuildConfigManager>().unwrap());
            let guild_cached = guild_id.to_guild_cached(&ctx).await.unwrap();
            if let Ok(guild_config) = gc_manager.get_guild_config(&guild_cached).await {
                if let Err(why) = new_member.add_roles(&ctx, guild_config.read().await.default_roles()).await {
                    println!("Failed to give roles to member {} of guild {}: {}", new_member.user.id, guild_id, why);
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
                    .title("Error during running a command")
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
        .event_handler(Handler{is_loop_running: AtomicBool::new(false)})
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
