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

// Whenever a state is changed
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

        debugger;
        for (var i = 0; i < userDB.users.length; i++) {
          let user = userDB.users[i]
          // Check to see if the user is in the list of users
          if (newState.member.id == user.id) {
            streamURL = user.tracks[0].track
            for (var j = 0; j < user.tracks.length; j++) {
              track = user.tracks[i]
              // Check current channelID to match with database ID
              if (track.channelID == newState.channelID) {
                streamURL = track.track
              }
            }
            stream = ytdl(streamURL);
            break;
          }
          else {
            console.log("User isn't in list of users\n")
          }
        }

        console.log("Playing %s", streamURL);

        // Play the stream here
        const dispatcher = connection.play(stream, streamOptions);

        // Leave after finish
        dispatcher.on("finish", () => {
          console.log("Welcome for %s ended", newState.member.displayName);
          voiceChannel.leave();
        });

        setTimeout(function() {
          console.log(
            "Welcome for %s elapsed for 10 seconds. Ending.",
            newState.member.displayName
          );
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

  // Set track command. allows user to manually set the track for their intro
  if (command === "settrack") {
    setTrackCommand()
  }
});

/* setTrackCommand:
 * -----------------------------------------------------------------------------
 * input: args -> Message
 *
 * This function will set the track for the user. It will parse the argument
 * passed into chat. Refer to the userDB.json file to understand the database
 * structure.
 */
function setTrackCommand(args) {
  let stream = args[0];
  // Search through list of user tracks
  for (var i = 0; i < userTracks.userTracks.length; i++) {
    // If the user is in the list of valid users, usertracks.json
    if (userTracks.userTracks[i].id === message.author.id) {
      // If it's the default
      if (stream === "default") {
        console.log("Set default message for %s", message.author.id);
        message.channel.send("You've set your welcome message to default");
        stream = defaultGreeting;
      }

      // If the track is not valid
      if (ytdl.validateURL(stream) === false) {
        console.log("Invalid %stream", stream);
        message.channel.send("That was an invalid link. Please try again.");
        break;
      }

      userTracks.userTracks[i].track = stream;
      console.log(
        `${userTracks.userTracks[i].id}: ${userTracks.userTracks[i].track}`
      );
      break;
    }
  }
  fs.writeFile(
    "usertracks.json",
    JSON.stringify(userTracks, null, 4),
    function(err, result) {
      if (err) console.log("error", err);
    }
  );
}
