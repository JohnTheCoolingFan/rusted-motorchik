use serenity::client::Context;
use serenity::model::channel::Message;
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::CommandResult;

static GITHUB_URL: &str = "https://github.com/JohnTheCoolingFan/rusted-motorchik";
static GITLAB_URL: &str = "Sorry, not available yet";

#[command]
async fn github(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(&ctx.http, GITHUB_URL).await?;
    Ok(())
}

#[command]
async fn gitlab(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(&ctx.http, GITLAB_URL).await?;
    Ok(())
}

#[command]
async fn source(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id.say(&ctx.http, format!("Choose whichever you want:\nGitHub: {}\nGitLab: {}", GITHUB_URL, GITLAB_URL)).await?;
    Ok(())
}

#[group]
#[commands(github, gitlab, source)]
struct Miscellaneous;
