const { SlashCommandBuilder, Routes } = require("discord.js");
const { REST } = require("@discordjs/rest");
const { clientId, guildId, token } = require("../config.json");

// List of slash commands and their descriptions for RiceBot
const commands = [
  new SlashCommandBuilder().setName("marco").setDescription("Replies with polo!"),
  new SlashCommandBuilder().setName("play").setDescription("Play a certain track"),
  new SlashCommandBuilder().setName("stop").setDescription("Stop a playing"),
].map(command => command.toJSON());

const rest = new REST({ version: "10" }).setToken(token);
rest.put(Routes.applicationGuildCommands(clientId, guildId), { body: commands })
  .then((data) => console.log(`Succesfully registered ${data.length}`
    + ` application commands.`))
  .catch(console.error);
