// Author: Lam Duong

// Dependencies listing
const { Client, Intents } = require("discord.js");
const { prefix, token } = require("../userSettings/config.json");
const ytdl = require("ytdl-core"); // For playing music on YT
var fs = require("fs");
const { userInfo } = require("os");

var defaultGreeting = "https://www.youtube.com/watch?v=i8a3gjt_Ar0";

// Login and authenticate BOT
const riceBot = new Client({ intents: [Intents.FLAGS.GUILDS] });

riceBot.login(token);

// Get list of tracks URLS
let rawUsersList = fs.readFileSync("userSettings/userDB.json");
let userDB = JSON.parse(rawUsersList);

// get list of users who do not have welcome messages, such as bots
let suppressedUsersList = fs.readFileSync("userSettings/suppressedUsers.json");
let suppressedUsers = JSON.parse(suppressedUsersList);

// get server settings
let serverSettingsFile = fs.readFileSync("userSettings/ricebotsettings.json");
let serverSettings = JSON.parse(serverSettingsFile);
let serverAdminID = serverSettings.admin;

function isSuppressed(memberID) {
  // check to see if the user is in list of suppressed users
  id = parseInt(memberID);
  for (var i = 0; i < suppressedUsers.length; i++) {
    if (id === suppressedUsers[i]) {
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
    isSuppressed(newState.member.id) === false
  ) {
    console.log(oldState.member.displayName + " joined the chat");
    var voiceChannel = newState.channel; // channel user just joined

    // Bot joins channel
    voiceChannel
      .join()
      .then((connection) => {
        let streamOptions = { seek: 0, volume: 1 };
        let stream = ytdl(defaultGreeting);
        let streamURL = "";

        for (var i = 0; i < userDB.users.length; i++) {
          let user = userDB.users[i];
          // Check to see if the user is in the list of users
          if (newState.member.id == user.id) {
            streamURL = user.tracks[0].track; // add default track first
            streamOptions = {
              seek: user.tracks[0].seek,
              volume: user.tracks[0].volume,
            };
            for (var j = 0; j < user.tracks.length; j++) {
              track = user.tracks[j];
              // Check current name to match with database ID
              if (track.channelName == newState.channel.name) {
                streamURL = track.track;
                streamOptions = { seek: track.seek, volume: track.volume };
                break;
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
  if (command === "settrack") {
    setTrackCommand(args, message);
  } else if (command === "help") {
    message.channel.send(helpCommand(args));
  } else if (command === "reset") {
    resetCommand(message);
  } else if (command === "volume") {
    message.channel.send(volumeCommand(args, message));
  } else if (command === "suppress") {
    let userFound = suppressCommand(args, message.author.id);
    if (userFound) {
      message.channel.send("You are no longer suppressed. Whew.");
    } else {
      message.channel.send("You are now suppressed. Bye.");
    }
  } else if (command === "suppressed") {
    if (isSuppressed(message.author.id) === true) {
      message.channel.send("You are currently suppressed. Pew pew.");
    } else {
      message.channel.send("You are not suppressed. Have fun!");
    }
  }
});

function helpCommand(args) {
  let result = ""
  const argSet = new Set(); // to prevent users from spamming multiple args.
  let about =
    "- This is a simple bot that allows you to have custom intros" +
    " whenever you join a voice channel.\n\n";
  let format =
    "Please type in an argument after the !help command for" +
    " help with specific functions. For example, the command below will show" +
    " help for using !settrack and !volume commands: ```!help settrack" +
    " volume```Possible arguments are: \n- all\n- settrack\n- volume\n-" +
    " suppress\n- suppressed\n ";
  let divider =
    "-------------------------------------------------------" +
    "-------------------------------------------------------\n";
  let setTrack =
    "**!settrack**: sets a track for your introduction. This" +
    " only works with valid YouTube links. It will also play an introduction " +
    "track for roughly 10 seconds.\n" +
    "Format: *!settrack [YouTubeURL] [channel name] -v*" +
    " *[volume level] -s [starting position in seconds]*  **Note:**" +
    " every field is optional except for YouTubeURL.\n\n" +
    "Example: This will set a default track for you in every channel unless" +
    " specified by other links." +
    " ```!settrack https://youtu.be/3WAOxKOmR90``` " +
    "Example 2: This will set your track to a YT link in Some Channel Name to" +
    " have a 10% volume and starting at 1 second.\n" +
    "```!settrack SomeYouTubeURL Some Channel Name -v 0.1 -s 1```\n";

  let volume =
    "**!volume**: sees what your current volume is in different voice" +
    " channels and allows you to set volume level.\n" +
    "Format: *!volume [channel name] [new volume]*  **Note**: fields are" +
    " optional\n\n Example: To see the volume of your current channels:" +
    "```!volume``` Example 2: To set the volume in Some Channel to 50%" +
    "```!volume Some Channel 0.5```";

  let suppress = "**!suppress**: toggles whether you want to have an intro or"
    + " not. This function will store you in the database of suppressed users"
    + " or it will remove you, depending on your status. Remember that this is "
    + "a toggling action. Use the !suppressed command to check if you are"
    + " suppressed or not.\n"

  let suppressed = "**!suppressed**: shows if you are suppressed or not. While"
    + " being suppressed, you will not have a voice intro in voice channels.\n"

  if (args.length === 0) {
    result = about + format;
  }
  else {
    for (var i = 0; i < args.length; i++) {
      if (!argSet.has(args[i])) {
        if (args[i] === "all") {
          result = about + divider + setTrack + divider + volume
            + divider + suppress + divider + suppressed;
          break; // break if there's an "all" argument included
        } else if (args[i] === "settrack") {
          result += divider + setTrack;
        } else if (args[i] === "volume") {
          result += divider + volume;
        } else if (args[i] === "suppress") {
          result += divider + suppress;
        } else if (args[i] === "suppressed") {
          result += divider + suppressed;
        }
      }
      argSet.add(args[i]);
    }
    if (result === "") { result = format; } // if no valid arguments provided.
  }
  return result;
}

/* resetCommand:
 * -----------------------------------------------------------------------------
 * input: message -> Discord.Message
 *
 * Reset all the tracks to the default track
 */
function resetCommand(message) {
  for (var userIndex = 0; userIndex < userDB.users.length; userIndex++) {
    if (userDB.users[userIndex].id === message.author.id) {
      newUserTrack = {
        channelName: "Default",
        track: defaultGreeting,
        seek: 0,
        volume: 1,
      };
      userDB.users[userIndex].tracks = [newUserTrack];

      // Write to JSON
      writeToJSON();
    }
  }
  message.channel.send("You've successfully reset your tracks");
}

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
  let channelName = "Default";
  startTime = 0; // in seconds
  volume = 1;

  // Handle different arguments
  if (args.length === 0) {
    message.channel.send(
      'Please follow the format -> "URL" "Channels\'' +
      ' Name". Type !help for usage and examples'
    );
  } else if (args.length >= 2) {
    channelName = getChannelName(args);
    if (channelName === "") channelName = "Default";
    startTime = getStartPosition(args);
    volume = getVolume(args);
    if (volume > 1 || volume < 0) {
      message.channel.send("Volume has to be between 0 and 1, try again");
    }
  }

  userFound = false;
  // Search through list of user tracks
  for (var i = 0; i < userDB.users.length; i++) {
    user = userDB.users[i];
    userID = message.author.id;

    // If the user is in the list of valid users, userDB.json
    if (user.id === userID) {
      userFound = true;

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
      found = false;
      for (j = 0; j < user.tracks.length; j++) {
        if (user.tracks[j].channelName === channelName) {
          user.tracks[j].track = streamURL;
          user.tracks[j].seek = startTime;
          user.tracks[j].volume = volume;
          found = true;
        }
      }
      if (found === false)
        // If it's a new channel being inserted
        user.tracks.push({
          channelName: channelName,
          track: streamURL,
          seek: startTime,
          volume: volume,
        });
      writeToJSON();
      break;
    }
  }
  if (userFound === false) {
    owner = message.member.guild.owner.displayName;
    message.channel.send(
      "You are not in the list of users to use RiceBot." +
      " Please contact " +
      owner
    );
  }
}

// Write to JSON file
function writeToJSON() {
  // Write to JSON file
  fs.writeFile(
    "userDB.json",
    JSON.stringify(userDB, null, 4),
    function(err, result) {
      if (err) console.log("error", err);
    }
  );
}

/* getChannelName
 *------------------------------------------------------------------------------
 * input: args
 *
 * Return the channel name of the string. If it encounters a flag such as -s or
 * -v, it will end. This is only used for setTrackCommand()
 */
function getChannelName(args) {
  channelName = "";
  // Start at 1 because args[0] is the URL
  for (var commandIndex = 1; commandIndex < args.length; commandIndex++) {
    if (args[commandIndex] == "-s" || args[commandIndex] == "-v") {
      break;
    } else {
      channelName += args[commandIndex];
      if (commandIndex !== args.length - 1) {
        if (args[commandIndex + 1] == "-s" || args[commandIndex + 1] == "-v") {
          break;
        } else {
          channelName += " ";
        }
      }
    }
  }
  return channelName;
}

/* getVolume
 *------------------------------------------------------------------------------
 * input: args
 *
 * Return the volume from the command. It searches for the '-v' flag. This is
 * used by the setTrackCommand()
 */
function getVolume(args) {
  volume = 1;
  for (var commandIndex = 1; commandIndex < args.length; commandIndex++) {
    if (args[commandIndex] === "-v") {
      volume = Number(args[commandIndex + 1]);
      break;
    }
  }
  return volume;
}

/* getVolume
 *------------------------------------------------------------------------------
 * input: args
 *
 * Return the starting position from the command. It searches for the '-v' flag
 */
function getStartPosition(args) {
  seek = 0;
  for (var commandIndex = 1; commandIndex < args.length; commandIndex++) {
    if (args[commandIndex] === "-s") {
      seek = Number(args[commandIndex + 1]);
      break;
    }
  }
  return seek;
}

/* volumeCommand()
 *--------------------------------------------------------------------------------
 * input: args
 *
 * Show the current volume. If there's a number that follows, then change that.
 */
function volumeCommand(args, message) {
  let response = "";
  let wrongFormat =
    "Please use the correct format: !volume [channel name]" +
    " [0 <= volume level <= 1] ";

  // If there are no arguments, just tell the user what their volume is
  if (args.length === 0) {
    for (var index = 0; index < userDB.users.length; index++) {
      if (message.author.id === userDB.users[index].id) {
        response = "Your current volume level is: \n";
        for (var j = 0; j < userDB.users[index].tracks.length; j++) {
          let track = userDB.users[index].tracks[j];
          if (track.channelName === "Default") {
            response += "Default: " + track.volume + "\n";
          } else {
            response += track.channelName + ": " + track.volume + "\n";
          }
        }
        break; // break the loop if the user is found
      }
      response =
        "You are not on the list of users to use RiceBot. Please contact " +
        "server admin.";
    }
  } else {
    let newVolumeString = args[args.length - 1];
    let newVolume = parseFloat(newVolumeString);
    let channelName = "";
    // Get channel name
    for (var k = 0; k < args.length - 1; k++) {
      channelName += args[k];
      if (k != args.length - 2) {
        channelName += " ";
      }
    }
    // Last argument has to be an int between 0 and 1.
    if (!isNaN(parseFloat(newVolume)) && newVolume >= 0 && newVolume <= 1) {
      for (var i = 0; i < userDB.users.length; i++) {
        // search users
        if (message.author.id === userDB.users[i].id) {
          // if user found
          for (var j = 0; j < userDB.users[i].tracks.length; j++) {
            if (channelName === userDB.users[i].tracks[j].channelName) {
              userDB.users[i].tracks[j].volume = newVolume;
              response =
                "You've set your volume in " + channelName + " to " + newVolume;
              writeToJSON();
              break;
            }
            response =
              "The channel, " +
              channelName +
              "was not found. Please try" +
              " again or type in !volume to show list of channels and their" +
              " corresponding volume.";
          }
          break;
        }
        response =
          "You are not on the list of users to use RiceBot. Please contact " +
          "server admin.";
      }
    } else {
      response =
        "The last argument is required to be a number between 0 and" +
        " 1, e.g. 0.88\n";
      response += wrongFormat;
    }
  }
  return response;
}

/*
 * suppress():
 * -----------------------------------------------------------------------------
 * args: the arguments passed in by the user
 * author: the user that sent this message
 */
function suppressCommand(args, authorID) {
  let userFound = false;
  let id = parseInt(authorID);
  for (var i = 0; i < suppressedUsers.length; i++) {
    if (id === suppressedUsers[i]) {
      userFound = true;

      // Remove user if found in list of suppressed users
      suppressedUsers.splice(i, 1);
      break;
    }
  }
  if (!userFound) {
    suppressedUsers.push(id);
  }
  fs.writeFile(
    "suppressedUsers.json",
    JSON.stringify(suppressedUsers, null, 4),
    function(err, result) {
      if (err) console.log("error", err);
    }
  );
  return userFound;
}

// TODO: Implement a function that allows user to cleanup channels that don't exist
function cleanUp() { }
