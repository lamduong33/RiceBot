const { SlashCommandBuilder } = require("discord.js");
const admins = require("../admins.json");

module.exports = {
  data: new SlashCommandBuilder()
    .setName("getAdmins")
    .setDescription("Show the current list of admins that can administer RiceBot."),
  async execute(interaction) {
    let response = "Here are the list of Ricebot admins:\n";
    admins.forEach(adminId => {
      let adminName = interaction.guild.members.fetch(adminId).displayName;
      response.concat(adminName);
    });
    await interaction.reply(response);
  },
};
