mod command_groups;
mod guild_config;

use command_groups::*;
use once_cell::sync::OnceCell;
use regex::Regex;

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
use serenity::utils::{ContentSafeOptions, ArgumentConvert};
use serenity::http::Http;
use guild_config::{GuildConfigManager, InfoChannelType};

const ROLE_QUEUE_INTERVAL: Duration = Duration::from_secs(30); // 30 seconds
const MOD_LIST_UPDATE_INTERVAL: Duration = Duration::from_secs(60 * 60); // 1 hour
const ERROR_EMBED_COLOR: (u8, u8, u8) = (255, 15, 15);

pub struct RoleQueue;

impl TypeMapKey for RoleQueue {
    type Value = Arc<RwLock<Vec<(GuildId, UserId)>>>;
}

// Info about ignoring certain users banned in a certain guild when handling guild_ban_addition
pub struct BanMessageIgnoreList;

impl TypeMapKey for BanMessageIgnoreList {
    type Value = Arc<RwLock<HashSet<(GuildId, UserId)>>>;
}

pub fn content_safe_settings(msg: &Message) -> ContentSafeOptions {
    match &msg.guild_id {
        Some(guild_id) => ContentSafeOptions::default().clean_channel(false).display_as_member_from(guild_id),
        _ => ContentSafeOptions::default().clean_channel(false).clean_role(false)
    }
}

struct Handler {
    is_loop_running: AtomicBool
}

impl Handler {
    async fn log_channel_kick_message(ctx: &Context, guild_id: GuildId, user: &UserId, kicked_by: &User, kick_reason: Option<String>) {
        if let Ok(guild_config) = GuildConfigManager::get_guild_config_from_ctx(ctx, guild_id).await {
            let log_ic_data_arc = guild_config.read().await.info_channels_data(InfoChannelType::Log);
            let log_ic_data = log_ic_data_arc.read().await;
            if log_ic_data.enabled {
                let channel = log_ic_data.channel_id;
                let reason = kick_reason.map(|r| format!("Reason: {}", r)).unwrap_or_else(|| "Reason was not provided".into());
                if let Err(why) = channel.say(ctx, format!("User {} was kicked by {}.\n{}", user, kicked_by, reason)).await {
                    println!("Error sending kick log message: {}", why);
                }
            }
        }
    }

    async fn log_channel_ban_message(ctx: &Context, guild_id: GuildId, user: &User, banned_by: Option<&User>, ban_reason: Option<String>) {
        let gc_manager = Arc::clone(ctx.data.read().await.get::<GuildConfigManager>().unwrap());
        let guild_cached = guild_id.to_guild_cached(&ctx).await.unwrap();
        if let Ok(guild_config) = gc_manager.get_cached_guild_config(&guild_cached).await {
            let log_ic_data_arc = guild_config.read().await.info_channels_data(InfoChannelType::Log);
            let log_ic_data = log_ic_data_arc.read().await;
            if log_ic_data.enabled {
                let channel = log_ic_data.channel_id;
                if let Ok(bans) = guild_cached.bans(ctx).await {
                    for ban in bans {
                        if &ban.user == user {
                            let reason = ban_reason.unwrap_or_else(|| ban.reason.map(|r| format!("Reason: {}", r)).unwrap_or_else(|| "Reason was not provided".into()));
                            let ban_issued_by = banned_by.map(|bby| format!(" by {}", bby)).unwrap_or_else(String::new);
                            if let Err(why) = channel.say(ctx, format!("User {} was banned{}.\n{}", user, ban_issued_by, reason)).await {
                                println!("Error sending a ban log message: {}", why);
                            }
                            break;
                        }
                    }
                }
            }
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    // Print account info
    async fn ready(&self, _: Context, ready: Ready) {
        println!("Connected as {}", ready.user.name);
    }

    // Redirect DMs to author
    async fn message(&self, ctx: Context, msg: Message) {
        if !msg.is_own(&ctx).await {
            // DM/PM redirect
            if msg.is_private() {
                if let Ok(appinfo) = ctx.http.get_current_application_info().await {
                    let owner = appinfo.owner;
                    if let Err(why) = owner.dm(&ctx.http, |m| m.content(format!("I have received a message from {}:\n{}", msg.author.tag(), msg.content))).await {
                        println!("Failed to redirect message: {}", why)
                    }
                }
            } else {
                // Send message preview in reply to a message with a link to a message
                {
                    static DISCORD_MESSAGE_LINK_REGEX: OnceCell<Regex> = OnceCell::new();
                    let links = DISCORD_MESSAGE_LINK_REGEX
                        .get_or_init(|| Regex::new(r"https://discord.com/channels/[0-9]*/[0-9]*/[0-9]*").unwrap())
                        .find_iter(&msg.content);
                    for link in links.map(|l| l.as_str()) {
                        if let Ok(message) = Message::convert(&ctx, None, None, link).await {
                            if let Err(why) = msg.channel_id
                                .send_message(&ctx, |cm| cm.reference_message(&msg)
                                    .embed(|e| e.author(|cea| cea
                                            .icon_url(message.author.avatar_url().unwrap_or_default())
                                            .name(message.author.name)
                                            .url(link))
                                    .description(message.content))
                                    .allowed_mentions(|cam| cam.empty_users().empty_roles().empty_parse())).await {
                                println!("Failed to reply: {}", why);
                            }
                        }
                    }
                }

                // Send mod info in response to a message with >>mod name<< pattern
                {
                    static MOD_NAME_REGEX: OnceCell<Regex> = OnceCell::new();
                    let mod_names = MOD_NAME_REGEX
                        .get_or_init(|| Regex::new(r">>[A-Za-z0-9 ]*<<").unwrap())
                        .find_iter(&msg.content);
                    for mod_name in mod_names.map(|mn| mn.as_str()) {
                        if let Err(why) = reply_process_mod(&ctx, &msg, &mod_name[2..(mod_name.len()-2)]).await {
                            println!("Failed to reply with mod data: {}", why);
                        }
                    }
                }
            }
        }
    }

    async fn cache_ready(&self, ctx: Context, _guilds: Vec<GuildId>) {
        println!("Cache built successfully!");

        let ctx = Arc::new(ctx);

        if !self.is_loop_running.load(Ordering::Relaxed) {
            let ctx1 = Arc::clone(&ctx);

            // Members on hold for role giving
            tokio::spawn(async move {
                loop {
                    let queue = Arc::clone(ctx1.data.read().await.get::<RoleQueue>().unwrap());
                    let mut new_queue: Vec<(GuildId, UserId)> = Vec::with_capacity(queue.read().await.len());
                    let gc_manager = Arc::clone(ctx1.data.read().await.get::<GuildConfigManager>().unwrap());
                    for item in &*queue.read().await {
                        if let Ok(mut member) = item.0.member(Arc::clone(&ctx1.http), item.1).await {
                            if member.pending {
                                new_queue.push(*item)
                            } else if let Ok(guild_config) = gc_manager.get_guild_config(member.guild_id, &ctx1).await {
                                if let Err(why) = member.add_roles(&ctx1, guild_config.read().await.default_roles()).await {
                                    println!("Failed to give roles to member {} of guild {}: {}", member.user.id, member.guild_id, why);
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

            // Update mod list
            tokio::spawn(async move {
                loop {
                    let gc_manager = Arc::clone(ctx2.data.read().await.get::<GuildConfigManager>().unwrap());
                    let guilds = ctx2.cache.guilds().await;
                    for guild in guilds {
                        if let Ok(guild_config) = gc_manager.get_guild_config(guild, &ctx2).await {
                            let mod_list_ic_data_arc = guild_config.read().await.info_channels_data(InfoChannelType::ModList);
                            let mod_list_ic_data = mod_list_ic_data_arc.read().await;
                            if mod_list_ic_data.enabled {
                                let channel = mod_list_ic_data.channel_id;
                                let mod_list_messages_arc = Arc::clone(&guild_config.read().await.mod_list_messages);
                                if mod_list_messages_arc.read().await.is_empty() {
                                    if let Ok(messages) = channel.messages(&ctx2, |gm| gm.limit(MOD_LIST.len() as u64)).await {
                                        if let Err(why) = channel.delete_messages(&ctx2, messages).await {
                                            println!("Failed to update mod list (delete old messages step) at guild {} in channel {} due to a following error: {}", guild, channel, why);
                                        }
                                    };
                                    match scheduled_modlist(ctx2.as_ref(), channel).await {
                                        Err(why) => println!("Failed to update mod list (send messages step) in guild {}, channel {} due to a following error: {}", guild, channel, why),
                                        Ok(message_ids) => {
                                            {
                                                let mut mod_list_messages = mod_list_messages_arc.write().await;
                                                mod_list_messages.clear();
                                                mod_list_messages.extend(message_ids);
                                            };
                                            if let Err(why) = guild_config.read().await.write().await {
                                                println!("Failed to write guild config for guild {}: {}", guild, why);
                                            }
                                        }
                                    }
                                } else if let Err(why) = update_mod_list(&ctx2, channel, guild, mod_list_messages_arc).await {
                                    println!("Failed to updte mod list (updating messages) in guild {}, channel {} due to a following error: {}", guild, channel, why);
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
        if let Ok(guild_config) = GuildConfigManager::get_guild_config_from_ctx(&ctx, guild_id).await {
            // Give roles or put on queue
            {
                let queue = Arc::clone(ctx.data.read().await.get::<RoleQueue>().unwrap());
                if !guild_config.read().await.default_roles().is_empty() {
                    if new_member.pending {
                        let mut queue_write = queue.write().await;
                        queue_write.push((guild_id, new_member.user.id))
                    } else if let Err(why) = new_member.add_roles(&ctx, guild_config.read().await.default_roles()).await {
                        println!("Failed to give roles to member {} of guild {}: {}", new_member.user.id, guild_id, why);
                    }
                }
            };
            // Send a welcoming message
            {
                let welcome_ic_data_arc = guild_config.read().await.info_channels_data(InfoChannelType::Welcome);
                let welcome_ic_data = welcome_ic_data_arc.read().await;
                if welcome_ic_data.enabled {
                    let channel = welcome_ic_data.channel_id;
                    if let Err(why) = channel.say(&ctx, format!("Welcome, {}!", new_member.mention())).await {
                        println!("Failed to greet member {} in guild {} due to a following error: {}", new_member, guild_id, why);
                    }
                }
            };
        }
    }

    // Member left a guild, or was banned/kicked
    async fn guild_member_removal(&self, ctx: Context, guild_id: GuildId, user: User, _member_data_if_available: Option<Member>) {
        if let Ok(guild_config) = GuildConfigManager::get_guild_config_from_ctx(&ctx, guild_id).await {
            let welcome_ic_data_arc = guild_config.read().await.info_channels_data(InfoChannelType::Welcome);
            let welcome_ic_data = welcome_ic_data_arc.read().await;
            if welcome_ic_data.enabled {
                let channel = welcome_ic_data.channel_id;
                if let Err(why) = channel.say(&ctx, format!("Goodbye, {}", user.name)).await {
                    println!("Failed to say goodbye to user {} who left guild {} due to a folloeing error: {}", user, guild_id, why);
                }
            }
        }
    }

    // Member was banned (by anyone, including this bot)
    async fn guild_ban_addition(&self, ctx: Context, guild_id: GuildId, banned_user: User) {
        let ignore_list = Arc::clone(ctx.data.read().await.get::<BanMessageIgnoreList>().unwrap());
        if ignore_list.read().await.contains(&(guild_id, banned_user.id)) {
            ignore_list.write().await.remove(&(guild_id, banned_user.id));
        } else {
            Self::log_channel_ban_message(&ctx, guild_id, &banned_user, None, None).await;
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

#[hook]
async fn before(ctx: &Context, msg: &Message, cmd_name: &str) -> bool {
    if let Ok(command_filter) = GuildConfigManager::get_command_filter_from_ctx(ctx, msg.guild_id.unwrap(), cmd_name).await {
        return command_filter.read().await.can_run(msg.channel_id).is_ok();
    }
    true
}

#[hook]
async fn after(ctx: &Context, msg: &Message, command_name: &str, command_result: CommandResult) {
    if let Err(why) = command_result {
        println!("Command '{}' returned error {}", command_name, why);
        if let Err(why_echo) = msg.channel_id.send_message(&ctx.http, |m| {
            m.add_embed(|e| {
                e.color(ERROR_EMBED_COLOR)
                    .title("Error executing a command")
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
        .before(before)
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
