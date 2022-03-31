#!/bin/bash

# A script to start bot using a config file
# For those who migrate from python version which used config.json

exec env DISCORD_TOKEN="$(jq -r '.token' config.json)" GUILD_CONFIG_HOME="$(jq -r '.json.dir' config.json)" "$@"
