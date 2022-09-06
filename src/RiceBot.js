// @ts-nocheck
// Require the necessary discord.js classes
const { Client, IntentsBitField, Collection } = require('discord.js');
const { token } = require('../config.json');
const fs = require("node:fs"); // Node's native file system module
const path = require("node:path"); // Nodes' native path utility module
const ytdl = require("ytdl-core"); // For playing music on YT

// NOTE: Add/remove intents HERE.
// For a list of intents, see:
// https://discord.com/developers/docs/topics/gateway#list-of-intents
const riceBotIntents = new IntentsBitField();
riceBotIntents.add(
    IntentsBitField.Flags.Guilds,
    IntentsBitField.Flags.GuildPresences,
    IntentsBitField.Flags.GuildMembers,
    IntentsBitField.Flags.GuildMessages,
    IntentsBitField.Flags.GuildVoiceStates,
    IntentsBitField.Flags.MessageContent,
);

const riceBot = new Client({ intents: riceBotIntents });
riceBot.login(token);

// Parsing commands for the bot
riceBot.commands = new Collection(); // Collection is a better Map class.
const commandsPath = path.join(__dirname, "commands");
const commandFiles = fs.readdirSync(commandsPath).filter(file => file.endsWith(".js"));
for (const file of commandFiles) {
    const filePath = path.join(commandsPath, file);
    const command = require(filePath);
    // Set a new item in the Collection
    // With the key as the command name and the value as the exported module
    riceBot.commands.set(command.data.name, command);
}

// Parsing events for the bot
const eventsPath = path.join(__dirname, "events");
const eventFiles = fs.readdirSync(eventsPath).filter(file => file.endsWith(".js"));
for (const eventFile of eventFiles) {
    const eventPath = path.join(eventsPath, eventFile);
    const event = require(eventPath);
    if (event.once) {
        riceBot.once(event.name, (...args) => event.execute(...args));
    } else {
        riceBot.on(event.name, (...args) => event.execute(...args));
    }
}

/**
 * Handles the slash commands given to RiceBot by the user.*/
riceBot.on("interactionCreate", async interaction => {
    if (!interaction.isChatInputCommand()) return;

    const command = interaction.client.commands.get(interaction.commandName);
    if (!command) return; // command is undefined when it's not a command
    try {
        await command.execute(interaction);
    }
    catch (error) {
        console.error(error);
        await interaction.reply({
            content: `Couldn't execute ${interaction.commandName}`,
            ephemeral: true,
        });
    }
});
