const { SlashCommandBuilder } = require("discord.js");
const admins = require("../admins.json");

module.exports = {
  data: new SlashCommandBuilder()
    .setName("getadmins")
    .setDescription("Show the current list of admins that can administer RiceBot."),
  async execute(interaction) {
    let response = "Here are the list of RiceBot admins:\n";
    admins.forEach(adminId => {
      let user = interaction.guild.members.cache.get(adminId).user;
      //response += `${user.toString()}\n`;
      response += `${user.username}#${user.discriminator}\n`;
    })
    await interaction.reply(response);
  },
};
