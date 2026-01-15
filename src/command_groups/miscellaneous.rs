use std::env;

use chrono::offset::Utc;
use serenity::{
    all::{CreateEmbed, CreateMessage},
    client::Context,
    framework::standard::{
        CommandResult,
        macros::{command, group},
    },
    model::channel::Message,
};

use crate::StartTime;

const GITHUB_URL: &str = "https://github.com/JohnTheCoolingFan/rusted-motorchik";
const GITLAB_URL: &str = "https://gitlab.com/JohnTheCoolingFan/rusted-motorchik";

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
    msg.channel_id
        .say(
            &ctx.http,
            format!("Choose whichever you want:\nGitHub: {GITHUB_URL}\nGitLab: {GITLAB_URL}"),
        )
        .await?;
    Ok(())
}

// TODO: Motorchik version, compiler version, etc
/// Info about host on which this bot is currently running
#[command]
async fn info(ctx: &Context, msg: &Message) -> CommandResult {
    let data_lock = ctx.data.read().await;
    let start_time = data_lock.get::<StartTime>().unwrap();
    let dur = Utc::now() - *start_time.as_ref();
    msg.channel_id
        .send_message(
            &ctx.http,
            CreateMessage::new().embed(
                CreateEmbed::new()
                    .title("Host info")
                    .timestamp(Utc::now())
                    .color((47, 137, 197))
                    .field(
                        "Hostname",
                        match sysinfo::System::host_name() {
                            Some(host) => host,
                            _ => "Unknown".into(),
                        },
                        true,
                    )
                    .field(
                        "Platform",
                        match sysinfo::System::long_os_version() {
                            Some(platform) => platform,
                            _ => "unknwon".into(),
                        },
                        true,
                    )
                    .field("Architecture", env::consts::ARCH, true)
                    .field(
                        "Bot uptime",
                        format!(
                            "{} days, {:02}:{:02}:{:02}",
                            dur.num_days(),
                            dur.num_hours() % 24,
                            dur.num_minutes() % 60,
                            dur.num_seconds() % 60
                        ),
                        true,
                    )
                    .field("Build git commit hash", env!("GIT_HASH"), true),
                /* TODO
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
            ),
        )
        .await?;
    Ok(())
}

// I'm not bringing "f***discord" command because it's about python and not quite relevant
// anymore...
/// Nothing of much interest
#[group]
#[commands(github, gitlab, source, info)]
struct Miscellaneous;
