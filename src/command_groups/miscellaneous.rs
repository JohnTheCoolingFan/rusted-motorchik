use std::env;
use serenity::client::Context;
use serenity::model::channel::Message;
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::CommandResult;
use chrono::offset::Utc;
use sysinfo::SystemExt;

const GITHUB_URL: &str = "https://github.com/JohnTheCoolingFan/rusted-motorchik";
const GITLAB_URL: &str = "Sorry, not available yet";

/// Send GitHub source link
#[command]
async fn github(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(&ctx.http, GITHUB_URL).await?;
    Ok(())
}

/// Send GitLab source link
#[command]
async fn gitlab(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(&ctx.http, GITLAB_URL).await?;
    Ok(())
}

/// Send source code links
#[command]
async fn source(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(&ctx.http, format!("Choose whichever you want:\nGitHub: {}\nGitLab: {}", GITHUB_URL, GITLAB_URL)).await?;
    Ok(())
}

// TODO: Motorchik version, compiler version, etc
/// Info about host on which this bot is currently running
#[command]
async fn hostinfo(ctx: &Context, msg: &Message) -> CommandResult {
    let system_info = sysinfo::System::default();
    msg.channel_id.send_message(&ctx.http, |m| {
        m.embed(|e| {
            e.title("Host info")
                .timestamp(&Utc::now())
                .color((47, 137, 197))
                .field("Hostname", match system_info.host_name() {
                    Some(host) => host,
                    _ => "Unknown".into()
                }, true)
                .field("Platform", match system_info.long_os_version() {
                    Some(platform) => platform,
                    _ => "unknwon".into()
                }, true)
                .field("Architecture", env::consts::ARCH, true)
                /*
                .field("Host uptime", match psutil::host::uptime() {
                    Ok(dur) => match chrono::Duration::from_std(dur) {
                        // TODO: Improve formatting (don't display days when 0, etc)
                        Ok(duration) => format!("{} day(s), {}:{}:{}",
                            duration.num_days(),
                            duration.num_hours() - duration.num_days()*24,
                            duration.num_minutes() - duration.num_hours()*60,
                            duration.num_seconds() - duration.num_minutes()*60),
                        _ => "Unknown".into()
                    },
                    _ => "Unknown".into()
                }, true)
                */
        })
    }).await?;
    Ok(())
}

// I'm not bringing "f***discord" command because it's about python and not quite relevant anymore...
/// Nothing of much interest
#[group]
#[commands(github, gitlab, source, hostinfo)]
struct Miscellaneous;
