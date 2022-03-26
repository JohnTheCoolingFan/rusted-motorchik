use serenity::prelude::*;
use serenity::model::prelude::*;
use std::sync::Arc;

pub struct RoleQueue;

impl TypeMapKey for RoleQueue {
    type Value = Arc<RwLock<Vec<(GuildId, UserId)>>>;
}
