use std::hash::Hash;
use std::fs::File;
use std::error::Error;
use std::iter;
use std::path::{PathBuf, Path};
use std::collections::HashMap;
use std::sync::Arc;
use serenity::builder::CreateEmbed;
use thiserror::Error;
use serenity::prelude::*;
use serenity::model::prelude::*;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use strum_macros::{EnumString, AsRefStr, EnumIter};
use strum::IntoEnumIterator;

#[cfg(test)]
use std::str::FromStr;

#[derive(Debug, EnumString, AsRefStr, Hash, Eq, PartialEq, Clone, Copy, Deserialize, Serialize, EnumIter)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum InfoChannelType {
    Welcome,
    Log,
    ModList
}

#[test]
fn info_channel_type_parse() {
    assert_eq!(InfoChannelType::Welcome, InfoChannelType::from_str("welcome").unwrap());
    assert_eq!(InfoChannelType::Log, InfoChannelType::from_str("log").unwrap());
    assert_eq!(InfoChannelType::ModList, InfoChannelType::from_str("mod-list").unwrap());
}

#[repr(u8)]
#[derive(Deserialize_repr, Serialize_repr, Hash, Eq, PartialEq, Clone, Copy)]
pub enum CommandDisability {
    None = 0,
    Global = 1,
    Blacklisted = 2,
    Whitelisted = 3
}

impl Default for CommandDisability {
    fn default() -> Self {
        Self::None
    }
}

pub struct GuildConfigManager {
    gc_cache: RwLock<HashMap<GuildId, Arc<RwLock<GuildConfig>>>>,
    config_path: PathBuf
}

impl TypeMapKey for GuildConfigManager {
    type Value = Arc<Self>;
}

impl GuildConfigManager {
    /// Inititalise GuildConfigManager
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        println!("Guild config home: {:?}", path);
        if !path.exists() {
            println!("{:?} doesn't exist, creating", path);
            std::fs::create_dir(&path).unwrap();
        }
        Self{gc_cache: RwLock::new(HashMap::new()), config_path: path}
    }

    /// Get guild config from manager
    pub async fn get_guild_config(&self, guild_id: GuildId, ctx: &Context) -> Result<Arc<RwLock<GuildConfig>>, Box<dyn Error + Send + Sync>> {
        self._get_guild_config((guild_id, ctx)).await
    }

    /// Get guild config from manager using a cached guild
    pub async fn get_cached_guild_config(&self, guild: &Guild) -> Result<Arc<RwLock<GuildConfig>>, Box<dyn Error + Send + Sync>> {
        self._get_guild_config(guild).await
    }

    /// Accepts either a combination of GuildId and &Context or &Guild
    async fn _get_guild_config(&self, guild: impl Into<GuildConfigGetArgs<'_>>) -> Result<Arc<RwLock<GuildConfig>>, Box<dyn Error + Send + Sync>> {
        let guild = guild.into();
        let guild_id = match guild {
            GuildConfigGetArgs::IdAndContext(id, _) => id,
            GuildConfigGetArgs::CachedGuild(g) => g.id
        };
        if !self.is_cached(guild_id).await {
            let mut gc_cache = self.gc_cache.write().await;
            if let Ok(gc) = GuildConfig::read(guild_id, &self.config_path) {
                gc_cache.insert(guild_id, Arc::new(RwLock::new(gc)));
            } else {
                match guild {
                    GuildConfigGetArgs::IdAndContext(id, ctx) => {
                        let guild_cached = id.to_guild_cached(ctx).await.ok_or(GuildConfigError::GuildCacheFailed(id))?;
                        gc_cache.insert(guild_id, Arc::new(RwLock::new(GuildConfig::new(&guild_cached, &self.config_path).await?)));
                    },
                    GuildConfigGetArgs::CachedGuild(guild_cached) => {
                        gc_cache.insert(guild_id, Arc::new(RwLock::new(GuildConfig::new(guild_cached, &self.config_path).await?)));
                    }
                };
            }
        }
        let gc_cache = self.gc_cache.read().await;
        Ok(Arc::clone(gc_cache.get(&guild_id).unwrap()))
    }

    /// Check if GuildCofnig is loaded into cache
    async fn is_cached(&self, guild_id: GuildId) -> bool {
        let gc_cache = self.gc_cache.read().await;
        gc_cache.contains_key(&guild_id)
    }

    /// Get GuildConfig with just a Context and guild id.
    /// Re-retrieves guild config manager for each call, if multiple guill configs are needed,
    /// better to use get_guild_config instead
    pub async fn get_guild_config_from_ctx(ctx: &Context, guild: GuildId) -> Result<Arc<RwLock<GuildConfig>>, Box<dyn Error + Send + Sync>> {
        let gc_manager = Arc::clone(ctx.data.read().await.get::<GuildConfigManager>().unwrap());
        gc_manager.get_guild_config(guild, ctx).await
    }

    pub async fn get_command_filter_from_ctx(ctx: &Context, guild: GuildId, command_name: &str) -> Result<Arc<RwLock<CommandFilter>>, Box<dyn Error + Send + Sync>> {
        let guild_config = Self::get_guild_config_from_ctx(ctx, guild).await?;
        let guild_config_read = guild_config.read().await;
        Ok(guild_config_read.get_command_filter(command_name).await)
    }
}

enum GuildConfigGetArgs<'a> {
    IdAndContext(GuildId, &'a Context),
    CachedGuild(&'a Guild)
}

impl<'a> From<(GuildId, &'a Context)> for GuildConfigGetArgs<'a> {
    fn from(args: (GuildId, &'a Context)) -> Self {
        Self::IdAndContext(args.0, args.1)
    }
}

impl<'a> From<&'a Guild> for GuildConfigGetArgs<'a> {
    fn from(guild: &'a Guild) -> Self {
        Self::CachedGuild(guild)
    }
}

pub struct GuildConfig {
    pub guild_id: GuildId,
    cf_cache: RwLock<HashMap<String, Arc<RwLock<CommandFilter>>>>,
    config_path: PathBuf,
    pub mod_list_messages: Arc<RwLock<Vec<(String, MessageId)>>>,
    pub message_link_lookup: bool,
    default_roles: Vec<RoleId>,
    info_channels: HashMap<InfoChannelType, Arc<RwLock<InfoChannelData>>>,
}

impl GuildConfig {
    /// Create new instance of GuildConfig
    async fn new(guild: &Guild, config_path: &Path) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let guild_config_data = GuildConfigData::new(guild.system_channel_id.unwrap_or(ChannelId(0)));
        let path = config_path.join(format!("guild_{}.json", guild.id));
        let result = Self::from_data(guild_config_data, guild.id, path);
        result.write().await?;
        Ok(result)
    }

    /// Read GuildConfig data from file and create Self
    fn read(guild_id: GuildId, config_path: &Path) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let path = config_path.join(format!("guild_{}.json", guild_id));
        let file = File::open(&path)?;
        let data = serde_json::from_reader(file)?;
        Ok(Self::from_data(data, guild_id, path))
    }

    /// Write GuildConfig to disk
    pub async fn write(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let data = self.to_data().await;
        let file = File::create(&self.config_path)?;
        serde_json::to_writer(file, &data)?;
        Ok(())
    }

    /// Create GuildConfig from deserialized data
    fn from_data(data: GuildConfigData, guild_id: GuildId, path: PathBuf) -> Self {
        Self {
            guild_id,
            config_path: path,
            cf_cache: RwLock::new(Self::hashmap_wrap_arcrwlock(data.command_filters)),
            mod_list_messages: Arc::new(RwLock::new(data.mod_list_messages)),
            message_link_lookup: data.message_link_lookup,
            default_roles: data.default_roles,
            info_channels: Self::hashmap_wrap_arcrwlock(data.info_channels)
        }
    }

    /// Helper function to wrap deserialized data in Arc<RwLock<>> (serde does not support tokio's
    /// async RwLock, sadly) 
    fn hashmap_wrap_arcrwlock<K, V>(mut hashmap: HashMap<K, V>) -> HashMap<K, Arc<RwLock<V>>>
    where
        K: Eq + Hash,
    {
        HashMap::from_iter(hashmap.drain().map(|(k, v)| (k, Arc::new(RwLock::new(v)))))
    }

    /// Create data object that can be easily serialized
    pub async fn to_data(&self) -> GuildConfigData {
        GuildConfigData {
            mod_list_messages: Vec::clone(&*self.mod_list_messages.read().await),
            message_link_lookup: self.message_link_lookup,
            default_roles: self.default_roles.clone(),
            info_channels: Self::unwrap_hashmap_arcrwlock(&self.info_channels).await,
            command_filters: {
                let command_filters = self.cf_cache.read().await;
                Self::unwrap_hashmap_arcrwlock(&*command_filters).await
            }
        }
    }

    /// Helper function to unwrap when writing
    async fn unwrap_hashmap_arcrwlock<K, V>(hashmap: &HashMap<K, Arc<RwLock<V>>>) -> HashMap<K, V>
    where
        K: Clone + Eq + Hash,
        V: Clone,
    {
        let mut result: HashMap<K, V> = HashMap::new();
        for (k, v) in hashmap {
            result.insert(k.clone(), v.read().await.clone());
        }
        result
    }

    /// Edit this GuildConfig
    /// If multiple edits are being made, it's better to collect collect them and apply all at once
    /// instead of editing small details sequentially
    pub async fn edit<F>(&mut self, f: F) -> Result<(), Box<dyn Error + Send + Sync>>
    where
        F: FnOnce(&mut EditGuildConfig) -> &mut EditGuildConfig
    {
        let mut edit_guild_config = EditGuildConfig::default();
        f(&mut edit_guild_config);
        if !(edit_guild_config.default_roles.is_none() && edit_guild_config.info_channels.is_empty()) {
            if let Some(def_roles) = edit_guild_config.default_roles {
                self.default_roles = def_roles;
            }
            for (ic_type, ic_edit) in edit_guild_config.info_channels {
                let mut ic_data = self.info_channels.get(&ic_type).unwrap().write().await;
                if let Some(state) = ic_edit.state {
                    ic_data.enabled = state
                }
                if let Some(channel) = ic_edit.channel {
                    ic_data.channel_id = channel
                }
                if ic_type == InfoChannelType::ModList {
                    self.mod_list_messages.write().await.clear();
                }
            }
            self.write().await
        } else {
            Ok(())
        }
    }

    /// Edit command filter for this guild and this name
    pub async fn edit_command_filter<F>(&mut self, command_name: &str, f: F) -> Result<(), Box<dyn Error + Send + Sync>>
    where
        F: FnOnce(&mut EditCommandFilter) -> &mut EditCommandFilter
    {
        let mut cf_edit = EditCommandFilter::default();
        f(&mut cf_edit);
        if cf_edit.filter_type.is_some() || cf_edit.channels.is_some() {
            {
                let command_filter_arc = self.get_command_filter(command_name).await;
                let mut command_filter = command_filter_arc.write().await;
                if let Some(filter_type) = cf_edit.filter_type {
                    command_filter.filter_type = filter_type
                }
                if let Some(channels) = cf_edit.channels {
                    command_filter.channels = channels
                }
            }
            self.write().await
        } else {
            Ok(())
        }
    }

    /// Accessor
    pub fn default_roles(&self) -> &Vec<RoleId> {
        &self.default_roles
    }
    
    /// Accessor
    pub fn info_channels_data(&self, info_channel: InfoChannelType) -> Arc<RwLock<InfoChannelData>> {
        Arc::clone(self.info_channels.get(&info_channel).unwrap())
    }

    /// Get command filter
    pub async fn get_command_filter(&self, command_name: &str) -> Arc<RwLock<CommandFilter>> {
        let mut cf_cache = self.cf_cache.write().await;
        let cf_entry = cf_cache.entry(command_name.into());
        Arc::clone(cf_entry.or_insert_with(|| Arc::new(RwLock::new(CommandFilter::default()))))
    }

    /// Create an embed with what parameters are set in this GuildConfig
    pub fn display_embed<'a, 'b>(&'a self, data: GuildConfigData, embed: &'b mut CreateEmbed) -> &'b mut CreateEmbed {
        data.display_embed(embed.title(format!("Config for guild {}", self.guild_id)))
    }
}

#[derive(Deserialize, Serialize)]
pub struct GuildConfigData {
    //guild_name: String,
    mod_list_messages: Vec<(String, MessageId)>,
    message_link_lookup: bool,
    default_roles: Vec<RoleId>,
    info_channels: HashMap<InfoChannelType, InfoChannelData>,
    command_filters: HashMap<String, CommandFilter>
}

impl GuildConfigData {
    fn default_info_channels(channel: ChannelId) -> HashMap<InfoChannelType, InfoChannelData> {
        HashMap::from_iter(InfoChannelType::iter()
            .zip(iter::repeat(InfoChannelData{channel_id:channel, enabled:false})))
    }

    fn new(default_channel: ChannelId) -> Self {
        Self{
            mod_list_messages: Vec::new(),
            message_link_lookup: true,
            default_roles: vec![],
            info_channels: Self::default_info_channels(default_channel),
            command_filters: HashMap::new()
        }
    }

    fn display_embed<'a, 'b>(&'a self, embed: &'b mut CreateEmbed) -> &'b mut CreateEmbed {
        embed
            .field("Message link lookup", match self.message_link_lookup {
                true => "Enabled",
                false => "Disabled"
            }, false)
            .field("Default roles", match self.default_roles.is_empty() {
                true => String::from("None"),
                false => {
                    self.default_roles.iter().map(|r| format!("{}", r.mention())).collect::<Vec<String>>().join(", ")
                }
            }, false)
            .field("Info channels", {
                self.info_channels.iter().map(|(ic_type, ic_data)| {
                    format!("**`{}`:**\n{}\nChannel: {}", ic_type.as_ref(), match ic_data.enabled {
                        true => "Enabled",
                        false => "Disabled"
                    }, ic_data.channel_id.mention())
                }).collect::<Vec<String>>().join("\n\n")
            }, false)
            .field("Command filters", {
                self.command_filters.iter().map(|(command_name, command_filter)| {
                    format!("**`{}`:**\n{}", command_name, match command_filter.filter_type() {
                        CommandDisability::None => "Enabled in all channels".into(),
                        CommandDisability::Global => "Disabled in all channels".into(),
                        _ => {
                            format!("{} in:\n{}", match command_filter.filter_type() {
                                CommandDisability::Blacklisted => String::from("Disabled"),
                                CommandDisability::Whitelisted => String::from("Enabled"),
                                _ => unreachable!()
                            }, command_filter.filter_list().iter().map(|c| format!("{}", c.mention())).collect::<Vec<String>>().join("\n"))
                        }
                    })
                }).collect::<Vec<String>>().join("\n\n")
            }, false)
    }
}

#[derive(Default)]
pub struct EditGuildConfig {
    default_roles: Option<Vec<RoleId>>,
    info_channels: HashMap<InfoChannelType, EditInfoChannel>
}

impl EditGuildConfig {
    /// Set default roles
    pub fn default_roles(&mut self, default_roles: Vec<RoleId>) -> &mut Self {
        self.default_roles = Some(default_roles);
        self
    }

    /// Set changes for InfoChannels
    pub fn info_channel<F>(&mut self, ic_type: InfoChannelType, f: F) -> &mut Self
    where
        F: FnOnce(&mut EditInfoChannel) -> &mut EditInfoChannel
    {
        let mut edit_ic = EditInfoChannel::default();
        f(&mut edit_ic);
        self.info_channels.insert(ic_type, edit_ic);
        self
    }

    #[allow(dead_code)]
    /// Alias for [Self::info_channel] with welcome channel dialed in
    pub fn welcome_info_channel<F>(&mut self, f: F) -> &mut Self
    where
        F: FnOnce(&mut EditInfoChannel) -> &mut EditInfoChannel
    {
        self.info_channel(InfoChannelType::Welcome, f)
    }

    #[allow(dead_code)]
    /// Alias for [Self::info_channel] with log channel dialed in
    pub fn log_info_channel<F>(&mut self, f: F) -> &mut Self
    where
        F: FnOnce(&mut EditInfoChannel) -> &mut EditInfoChannel
    {
        self.info_channel(InfoChannelType::Log, f)
    }

    #[allow(dead_code)]
    /// Alias for [Self::info_channel] with modlist channel dialed in
    pub fn modlist_info_channel<F>(&mut self, f: F) -> &mut Self
    where
        F: FnOnce(&mut EditInfoChannel) -> &mut EditInfoChannel
    {
        self.info_channel(InfoChannelType::ModList, f)
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct InfoChannelData {
    pub channel_id: ChannelId,
    pub enabled: bool
}

#[derive(Default)]
pub struct EditInfoChannel {
    state: Option<bool>,
    channel: Option<ChannelId>
}

impl EditInfoChannel {
    /// Set new state
    pub fn state(&mut self, new_state: bool) -> &mut Self {
        self.state = Some(new_state);
        self
    }

    /// Set new channel
    pub fn channel(&mut self, new_channel: ChannelId) -> &mut Self {
        self.channel = Some(new_channel);
        self
    }
}

#[derive(Default, Deserialize, Serialize, Clone)]
pub struct CommandFilter {
    #[serde(rename = "type")]
    filter_type: CommandDisability,
    channels: Vec<ChannelId>,
}

impl CommandFilter {
    pub fn can_run(&self, channel_id: ChannelId) -> std::result::Result<CommandDisability, CommandDisability> {
        match self.filter_type() {
            CommandDisability::None => Ok(CommandDisability::None),
            CommandDisability::Global => Err(CommandDisability::Global),
            CommandDisability::Blacklisted => {
                match self.filter_list().contains(&channel_id) {
                    true => Err(CommandDisability::Blacklisted),
                    false => Ok(CommandDisability::Blacklisted)
                }
            },
            CommandDisability::Whitelisted => {
                match self.filter_list().contains(&channel_id) {
                    true => Ok(CommandDisability::Whitelisted),
                    false => Err(CommandDisability::Whitelisted)
                }
            }
        }
    }

    pub fn filter_type(&self) -> CommandDisability {
        self.filter_type
    }

    pub fn filter_list(&self) -> &Vec<ChannelId> {
        &self.channels
    }
}

#[derive(Default)]
pub struct EditCommandFilter {
    filter_type: Option<CommandDisability>,
    channels: Option<Vec<ChannelId>>
}

impl EditCommandFilter {
    /// Set filter type
    pub fn filter_type(&mut self, filter_type: CommandDisability) -> &mut Self {
        self.filter_type = Some(filter_type);
        self
    }

    /// Set filter list
    pub fn channels(&mut self, channels: Vec<ChannelId>) -> &mut Self {
        self.channels = Some(channels);
        self
    }

    /// Shorthand for setting filter_type to CommandDisability::None
    pub fn enable(&mut self) -> &mut Self {
        self.filter_type(CommandDisability::None)
    }

    pub fn disable(&mut self) -> &mut Self {
        self.filter_type(CommandDisability::Global)
    }
}

#[derive(Debug, Error)]
pub enum GuildConfigError {
    #[error("Failed to fetch guild {0} from cache")]
    GuildCacheFailed(GuildId)
}
