use serenity::model::prelude::*;
use serenity::prelude::*;
use std::sync::Arc;

#[derive(Clone)]
pub struct RoleQueueItem {
    pub user_id: UserId,
    pub guild_id: GuildId,
    pub roles: Vec<RoleId>,
}

pub struct RoleQueue;

impl TypeMapKey for RoleQueue {
    type Value = Arc<RwLock<Vec<RoleQueueItem>>>;
}
