const { Routes } = require("discord.js");
const { REST } = require("@discordjs/rest");
const fs = require("node:fs"); // Node's native file system module
const path = require("node:path"); // Nodes' native path utility module
const { clientId, guildId, token } = require("../config.json");

// Parse the commands using paths to command files.
const commands = [];
const commandsPath = path.join(__dirname, "commands");
const commandFiles = fs.readdirSync(commandsPath).filter(file => file.endsWith(".js"));
for (const file of commandFiles) {
  const filePath = path.join(commandsPath, file);
  const command = require(filePath);
  commands.push(command.data.toJSON());
}

const rest = new REST({ version: "10" }).setToken(token);
rest.put(Routes.applicationGuildCommands(clientId, guildId), { body: commands })
  .catch(console.error);
