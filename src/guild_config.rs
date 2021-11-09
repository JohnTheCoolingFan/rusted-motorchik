use std::hash::Hash;
use std::fs::File;
use std::error::Error;
use std::iter;
use std::path::{PathBuf, Path};
use std::collections::HashMap;
use serenity::prelude::*;
use serenity::model::prelude::*;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use strum_macros::{EnumString, AsRefStr, EnumIter};
use strum::IntoEnumIterator;
use tokio::sync::RwLockReadGuard;

pub struct GuildConfigManagerKey;

impl TypeMapKey for GuildConfigManagerKey {
    type Value = GuildConfigManager;
}

#[derive(EnumString, AsRefStr, Hash, Eq, PartialEq, Clone, Copy, Deserialize, Serialize, EnumIter)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum InfoChannelType {
    Welcome,
    Log,
    ModList
}

pub struct GuildConfigManager {
    gc_cache: RwLock<HashMap<GuildId, RwLock<GuildConfig>>>,
    config_path: PathBuf
}

pub struct GuildConfigReadLock<'a, T: Send + Sync, K: Send + Sync + Eq + Hash>(pub K, RwLockReadGuard<'a, HashMap<K, RwLock<T>>>);

impl<'a, T: Send + Sync, K: Send + Sync + Eq + Hash> GuildConfigReadLock<'a, T, K> {
    pub fn get(&self) -> &RwLock<T> {
        self.1.get(&self.0).unwrap()
    }
}

impl GuildConfigManager {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self{gc_cache: RwLock::new(HashMap::new()), config_path: path.into()}
    }

    pub async fn get_guild_config(&self, guild: &Guild) -> Result<GuildConfigReadLock<'_, GuildConfig, GuildId>, Box<dyn Error + Send + Sync>> {
        let gc_cache = self.gc_cache.read().await;
        if gc_cache.contains_key(&guild.id) {
            Ok(GuildConfigReadLock(guild.id, gc_cache))
        } else {
            let mut gc_cache = self.gc_cache.write().await;
            gc_cache.insert(guild.id, RwLock::new(GuildConfig::new(guild, &self.config_path)?));
            let gc_cache = self.gc_cache.read().await;
            Ok(GuildConfigReadLock(guild.id, gc_cache))
        }
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
    pub fn info_channel<F: FnOnce(&mut EditInfoChannel) -> &mut EditInfoChannel>(&mut self, ic_type: InfoChannelType, f: F) -> &mut Self {
        let mut edit_ic = EditInfoChannel::default();
        f(&mut edit_ic);
        self.info_channels.insert(ic_type, edit_ic);
        self
    }

    /// Alias for [Self::info_channel] with welcome channel dialed in
    pub fn welcome_info_channel<F: FnOnce(&mut EditInfoChannel) -> &mut EditInfoChannel>(&mut self, f: F) -> &mut Self {
        self.info_channel(InfoChannelType::Welcome, f)
    }

    /// Alias for [Self::info_channel] with log channel dialed in
    pub fn log_info_channel<F: FnOnce(&mut EditInfoChannel) -> &mut EditInfoChannel>(&mut self, f: F) -> &mut Self {
        self.info_channel(InfoChannelType::Log, f)
    }

    /// Alias for [Self::info_channel] with modlist channel dialed in
    pub fn modlist_info_channel<F: FnOnce(&mut EditInfoChannel) -> &mut EditInfoChannel>(&mut self, f: F) -> &mut Self {
        self.info_channel(InfoChannelType::ModList, f)
    }
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

pub struct GuildConfig {
    pub guild_id: GuildId,
    cf_cache: RwLock<HashMap<String, RwLock<CommandFilter>>>,
    config_path: PathBuf,
    data: GuildConfigData
}

impl GuildConfig {
    /// Accessor
    pub fn default_roles(&self) -> &Vec<RoleId> {
        &self.data.default_roles
    }
    
    /// Accessor
    pub fn info_channels_data(&self, info_channel: InfoChannelType) -> Option<&InfoChannelData> {
        self.data.info_channels.get(&info_channel)
    }

    /// Get command filter
    pub async fn get_command_filter(&self, command_name: &str) -> GuildConfigReadLock<'_, CommandFilter, String> {
        let cf_cache = self.cf_cache.read().await;
        if cf_cache.contains_key(command_name) {
            GuildConfigReadLock(command_name.into(), cf_cache)
        } else {
            let mut cf_cache = self.cf_cache.write().await;
            cf_cache.insert(command_name.into(), RwLock::new(CommandFilter::default(self.guild_id, command_name.into())));
            let cf_cache = self.cf_cache.read().await;
            GuildConfigReadLock(command_name.into(), cf_cache)
        }
    }

    /// Edit this GuildConfig
    /// If multiple edits are being made, it's better to collect collect them and apply all at once
    /// instead of editing small details sequentially
    pub async fn edit<F: FnOnce(&mut EditGuildConfig) -> &mut EditGuildConfig>(&mut self, f: F) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut edit_guild_config = EditGuildConfig::default();
        f(&mut edit_guild_config);
        if !(edit_guild_config.default_roles.is_none() && edit_guild_config.info_channels.is_empty()) {
            if let Some(def_roles) = edit_guild_config.default_roles {
                self.data.default_roles = def_roles;
            }
            for ic_edit in edit_guild_config.info_channels {
                let ic_data = self.data.info_channels.get_mut(&ic_edit.0).unwrap();
                if let Some(state) = ic_edit.1.state {
                    ic_data.enabled = state
                }
                if let Some(channel) = ic_edit.1.channel {
                    ic_data.channel_id = channel
                }
            }
            self.write()
        } else {
            Ok(())
        }
    }

    /// Edit command filter for this guild and this name
    pub async fn edit_command_filter<F: FnOnce(&mut EditCommandFilter) -> &mut EditCommandFilter>(&mut self, command_name: &str, f: F) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut cf_edit = EditCommandFilter::default();
        f(&mut cf_edit);
        if cf_edit.filter_type.is_some() && cf_edit.channels.is_some() {
            let command_filter_lock = self.get_command_filter(command_name).await;
            let mut command_filter = command_filter_lock.get().write().await;
            if let Some(filter_type) = cf_edit.filter_type {
                command_filter.data.filter_type = filter_type
            }
            if let Some(channels) = cf_edit.channels {
                command_filter.data.channels = channels
            }
            self.write()
        } else {
            Ok(())
        }
    }


    /// Create new instance of GuildConfig
    fn new(guild: &Guild, config_path: &Path) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let guild_config_data = GuildConfigData::new(guild.system_channel_id.unwrap_or(ChannelId(0)));
        let path = config_path.join(format!("guild_{}.json", guild.id));
        let result = Self{guild_id:guild.id, config_path:path, data:guild_config_data, cf_cache:RwLock::new(HashMap::new())};
        result.write()?;
        Ok(result)
    }

    /// Write GuildConfig to disk
    fn write(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let file = match File::open(&self.config_path) {
            Ok(f) => f,
            Err(_) => File::create(&self.config_path)?
        };
        serde_json::to_writer(file, &self.data)?;
        Ok(())
    }

    /// Read GuildConfig data from file and create Self
    fn read(guild_id: GuildId, config_path: &Path) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let path = config_path.join(format!("guild_{}.json", guild_id));
        let file = File::open(&path)?;
        let data = serde_json::from_reader(file)?;
        Ok(Self{guild_id, cf_cache:RwLock::new(HashMap::new()), config_path:path, data})
    }

    /// Update GuildConfig from file
    async fn update_self(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let file = File::open(&self.config_path)?;
        let new_data = serde_json::from_reader(file)?;
        self.data = new_data;
        self.cf_cache.write().await.clear();
        Ok(())
    }
}

#[derive(Deserialize, Serialize)]
struct GuildConfigData {
    //guild_name: String,
    default_roles: Vec<RoleId>,
    info_channels: HashMap<InfoChannelType, InfoChannelData>,
    command_filters: HashMap<String, CommandFilterData>
}

impl GuildConfigData {
    fn default_info_channels(channel: ChannelId) -> HashMap<InfoChannelType, InfoChannelData> {
        HashMap::from_iter(InfoChannelType::iter()
            .zip(iter::repeat(InfoChannelData{channel_id:channel, enabled:false})))
    }

    fn new(default_channel: ChannelId) -> Self {
        Self{
            default_roles: vec![],
            info_channels: Self::default_info_channels(default_channel),
            command_filters: HashMap::new()
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct InfoChannelData {
    pub channel_id: ChannelId,
    pub enabled: bool
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

pub struct CommandFilter {
    pub guild_id: GuildId,
    pub command_name: String,
    data: CommandFilterData
}

impl CommandFilter {
    pub fn can_run(&self, channel_id: ChannelId) -> Result<CommandDisability, CommandDisability> {
        match self.filter_type() {
            CommandDisability::None => Ok(CommandDisability::None),
            CommandDisability::Global => Err(CommandDisability::Global),
            CommandDisability::Blacklisted => {
                match self.filter_list().binary_search(&channel_id) {
                    Ok(_) => Err(CommandDisability::Blacklisted),
                    Err(_) => Ok(CommandDisability::Blacklisted)
                }
            },
            CommandDisability::Whitelisted => {
                match self.filter_list().binary_search(&channel_id) {
                    Ok(_) => Ok(CommandDisability::Whitelisted),
                    Err(_) => Err(CommandDisability::Whitelisted)
                }
            }
        }
    }

    fn default(guild_id: GuildId, command_name: String) -> Self {
        Self{guild_id, command_name, data:CommandFilterData::default()}
    }

    pub fn filter_type(&self) -> CommandDisability {
        self.data.filter_type
    }

    pub fn filter_list(&self) -> &Vec<ChannelId> {
        &self.data.channels
    }
}

#[derive(Default, Deserialize, Serialize)]
struct CommandFilterData {
    #[serde(rename = "type")]
    filter_type: CommandDisability,
    channels: Vec<ChannelId>,
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
