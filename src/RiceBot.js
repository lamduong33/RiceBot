// @ts-nocheck
// Require the necessary discord.js classes
const { Client, IntentsBitField } = require('discord.js');
const { token } = require('../config.json');
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

riceBot.on('interactionCreate', async interaction => {
    if (!interaction.isChatInputCommand()) return;

    const { commandName } = interaction;

    if (commandName === 'marco') {
        await interaction.reply('Pong!');
    }
});


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
