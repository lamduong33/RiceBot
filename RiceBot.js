// Dependencies listing
const Discord = require("discord.js");
const { prefix, token } = require("./config.json");
const ytdl = require("ytdl-core"); // For playing music on YT
var fs = require("fs");

// WELCOME TO THE RICE FIELDS
var defaultGreeting = "https://www.youtube.com/watch?v=i8a3gjt_Ar0";

// Login and authenticate BOT
const riceBot = new Discord.Client();
riceBot.login(token);

// Get list of tracks URLS
let rawUsersList = fs.readFileSync("userDB.json");
let userDB = JSON.parse(rawUsersList);


// get list of users who do not have welcome messages, such as bots
let botsListFile = fs.readFileSync("botslist.json");
let bots = JSON.parse(botsListFile);

// get server settings
let serverSettingsFile = fs.readFileSync("ricebotsettings.json")
let serverSettings = JSON.parse(serverSettingsFile)
let serverAdminID = serverSettings.admin


function inBotsList(memberID) {
  // check to see if the user is in list of bots
  for (var i = 0; i < bots.botsList.length; i++) {
    if (memberID === bots.botsList[i].id) {
      return true;
    }
  }
  return false;
}

// listener and logging
riceBot.once("ready", () => {
  console.log("RiceBot is ready!");
});
riceBot.once("reconnecting", () => {
  console.log("RiceBot is reconnecting...");
});
riceBot.once("disconnect", () => {
  console.log("RiceBot is disconnected!!");
});
riceBot.once("error", () => {
  console.log("error!!!");
});


// read commands from user messages
riceBot.on("message", async (message) => {
  if (message.author.bot) return; // ignore message if it's from the bot
  if (!message.content.startsWith(prefix)) return;
}); // assign message

/* Where the magic happens. This allows the bot to listen on the state of the
 * voice channel. Once it gets updated, we will then check the new state and see
 * who joins, then play the track.
 */
riceBot.on("voiceStateUpdate", (oldState, newState) => {
  // Check if a new user has joined the channel due to channel's state change.
  if (
    newState.channelID !== null &&
    oldState.channelID === null &&
    inBotsList(newState.member.id) === false
  ) {
    console.log(oldState.member.displayName + " joined the chat");
    var voiceChannel = newState.channel; // channel user just joined

    // Bot joins channel
    // TODO: Add the ability to not play banned uses' sound clips
    voiceChannel
      .join()
      .then((connection) => {
        let streamOptions = { seek: 0, volume: 1 };
        let stream = ytdl(defaultGreeting);
        let streamURL = "";

        for (var i = 0; i < userDB.users.length; i++) {
          let user = userDB.users[i]
          // Check to see if the user is in the list of users
          if (newState.member.id == user.id) {
            streamURL = user.tracks[0].track
            for (var j = 0; j < user.tracks.length; j++) {
              track = user.tracks[j]
              // Check current name to match with database ID
              if (track.channelName == newState.channel.name) {
                streamURL = track.track
              }
            }
            stream = ytdl(streamURL);
            break;
          }
        }

        // Play the stream here
        const dispatcher = connection.play(stream, streamOptions);

        // Leave after finish
        dispatcher.on("finish", () => {
          voiceChannel.leave();
        });

        setTimeout(function() {
          voiceChannel.leave();
        }, 10000);
      })
      .catch((err) => console.log(err));
  } else if (newState.channelID === null && oldState.channelID !== null) {
    console.log(newState.member.displayName + " left the chat");
  }
});

// COMMANDS FOR RICEBOT
riceBot.on("message", (message) => {
  // Format the command to lowercase
  if (!message.content.startsWith(prefix) || message.author.bot) return;
  const args = message.content.slice(prefix.length).split(" ");
  const command = args.shift().toLowerCase();

  // Interpret different commands
  if (command == "settrack") {
    setTrackCommand(args, message)
  }
});

/* setTrackCommand:
 * -----------------------------------------------------------------------------
 * input: args -> string[]
 *        message -> Discord.Message
 *
 * This function will set the DEFAULT track for user. It will parse the argument
 * passed into chat. Refer to the userDB.json file to understand the database
 * structure.
 */
function setTrackCommand(args, message) {
  let streamURL = args[0];
  let channelName = "0"

  // Handle different arguments
  if (args.length <= 2) {
    channelName = args[1]
  } else {
    channelName = ""
    // Append the rest of channels name
    for (var nameIndex = 1; nameIndex < args.length; nameIndex++){
      channelName += args[nameIndex]
      if (nameIndex !== args.length-1)
        channelName += " "
      }
  }

  userFound = false
  // Search through list of user tracks
  for (var i = 0; i < userDB.users.length; i++) {

    user = userDB.users[i]
    userID = message.author.id

    // If the user is in the list of valid users, userDB.json
    if (user.id === userID) {
      userFound = true

      // If it's the default
      if (streamURL === "default") {
        console.log("Set default message for %s", userID);
        message.channel.send("You've set your welcome message to default");
        streamURL = defaultGreeting;
      }
      // If the track is not valid
      if (ytdl.validateURL(streamURL) === false) {
        console.log("Invalid stream URL: %s", streamURL);
        message.channel.send("That was an invalid link. Please try again.");
        break;
      }

      // Set the stream and write to JSON file
      found = false
      for (j = 0; j < user.tracks.length; j++) {
        if (user.tracks[j].channelName === channelName) {
          user.tracks[j].track = streamURL
          found = true
        }
      }
      if (found === false) // If it's a new channel being inserted
        user.tracks.push({ "channelName": channelName, "track": streamURL })

      // Write to JSON file
      fs.writeFile(
        "userDB.json",
        JSON.stringify(userDB, null, 4),
        function(err, result) {
          if (err) console.log("error", err);
        }
      );
      break;
    }
  }
  if (userFound === false) {
    owner = message.member.guild.owner.displayName
    message.channel.send("You are not in the list of users to use RiceBot." +
      " Please contact " + owner)
  }
}
