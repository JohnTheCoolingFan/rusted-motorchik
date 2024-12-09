use std::ops::Deref;

use serenity::{
    all::EditMessage,
    client::Context,
    framework::standard::{
        macros::{command, group},
        CommandResult,
    },
    model::channel::Message,
};

/// You spin my head right round...
#[command]
#[max_args(0)]
async fn spin(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id
        .say(&ctx.http, "https://www.youtube.com/watch?v=PGNiXGX2nLU")
        .await?;
    Ok(())
}

/// Obvious.
#[command]
#[aliases(XcQ)]
async fn rickroll(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id
        .say(
            &ctx.http,
            "<https://www.youtube.com/watch?v=dQw4w9WgXcQ>\n<:kappa_jtcf:546748910765604875>",
        )
        .await?;
    Ok(())
}

#[command]
#[aliases(UDOD_COMMUNIST, UDOD, udod, УДОД_КОММУНИСТ, удод_коммунист, УДОД, удод)]
async fn udod_communist(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id
        .say(&ctx.http, "https://youtu.be/OhqSg660cP8")
        .await?;
    Ok(())
}

#[command]
#[aliases(
    UDOD_COMMUNIST_2,
    UDOD2,
    udod2,
    УДОД_КОММУНИСТ_2,
    удод_коммунист_2,
    УДОД2,
    удод2
)]
async fn udod_communist_2(ctx: &Context, msg: &Message) -> CommandResult {
    msg.channel_id
        .say(&ctx.http, "https://youtu.be/BgF5HcnNN-Q")
        .await?;
    Ok(())
}

#[command]
/// "Wanna play a little game of pong?")]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    let mut pong = msg.channel_id.say(&ctx.http, "pong!").await?;
    let time_diff = *pong.timestamp.deref() - *msg.timestamp.deref();
    let time_diff_ms: f64 = {
        match time_diff.num_microseconds() {
            Some(us) => (us as f64) / 1000.0,
            _ => time_diff.num_milliseconds() as f64,
        }
    };
    pong.edit(
        &ctx.http,
        EditMessage::new().content(format!("pong!\nTime delta is {time_diff_ms} ms")),
    )
    .await?;
    Ok(())
}

/// Nothing useful
#[group]
#[commands(spin, rickroll, udod_communist, udod_communist_2, ping)]
#[summary("F U N")]
struct FunCommands;
