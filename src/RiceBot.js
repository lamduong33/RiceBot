// @ts-nocheck
// Require the necessary discord.js classes
const { Client, IntentsBitField, Collection } = require('discord.js');
const { token } = require('../config.json');
const fs = require("node:fs"); // Node's native file system module
const path = require("node:path"); // Nodes' native path utility module
// const ytdl = require("ytdl-core"); // For playing music on YT

// NOTE: Add/remove intents HERE.
// For a list of intents, see:
// https://discord.com/developers/docs/topics/gateway#list-of-intents
const riceBotIntents = new IntentsBitField();
riceBotIntents.add(
    IntentsBitField.Flags.Guilds,
    IntentsBitField.Flags.GuildPresences,
    IntentsBitField.Flags.GuildMembers,
    IntentsBitField.Flags.GuildMessages,
    IntentsBitField.Flags.MessageContent,
);

const riceBot = new Client({ intents: riceBotIntents });

// The default message
// const welcomeToTheRiceFields = "https://www.youtube.com/watch?v=i8a3gjt_Ar0";

// When the client is ready, run this code (only once)
riceBot.once('ready', () => {
    console.log('Ready!');
});

// Running commands for the bot
riceBot.commands = new Collection(); // Collection is a better Map class.
const commandsPath = path.join(__dirname, "commands");
const commandFiles = fs.readdirSync(commandsPath).filter(file => file.endsWith(".js"));
for (const file of commandFiles) {
    const filePath = path.join(commandsPath, file);
    const command = require(filePath);
    riceBot.commands.set(command.data.name, command);
}

/**
 * Greet the user here.*/
riceBot.on("voiceStateUpdate", (oldState, newState) => {
    // Check if a new user has joined the channel due to channel's state change.
    console.log(newState);
    if (newState.channelId !== null && oldState.channelId === null) {
        console.log(oldState.member.displayName + " joined the chat");
    }
    else if (newState.channelId === null && oldState.channelId !== null) {
        console.log(newState.member.displayName + " left the chat");
    }
});

// Login to Discord with your client's token
riceBot.login(token);
