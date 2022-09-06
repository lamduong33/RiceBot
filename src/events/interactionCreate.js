// This event listener will simply log any commands the users pass in.
const fs = require("node:fs");
const path = require("node:path");
const logFile = path.join(__dirname, "../../logs", "commands.log")
const time = new Date();


/**
 * @brief convert the day of the week as a number and return it as a string
 * @param dayNumber the day as an integer, e.g. 1, 2, or 3. 0 = Sunday.
 * @returns the day as a string, e.g. "WED".*/
function getDayOfTheWeek(dayNumber) {
  switch(dayNumber) {
    case 0 :
      return "SUN";
    case 1 :
      return "MON";
    case 2 :
      return "TUE";
    case 3 :
      return "WED";
    case 4 :
      return "THU";
    case 5 :
      return "FRI";
    case 6 :
      return "SAT";
  }
}

module.exports = {
  name: "interactionCreate",
  execute(interaction) {
    const userAction = (`${interaction.user.tag} in #${interaction.channel.name}`
      + ` triggered the /${interaction.commandName} interaction`);
    const currentTime =
          `${time.getUTCHours()}:${time.getUTCMinutes()}:${time.getUTCSeconds()}`
          + ` on ${getDayOfTheWeek(time.getUTCDay())}-${time.getUTCDate()}`
          + `/${time.getUTCMonth()}/${time.getUTCFullYear()} UTC`;
    const result = `${userAction} at ${currentTime}\n`;
    fs.writeFile(logFile, result, { flag: 'a+' }, err => {});
  },
};
