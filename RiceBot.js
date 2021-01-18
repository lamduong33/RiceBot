// Dependencies listing
const Discord = require("discord.js");
const
{
    prefix,
    token
} = require("./config.json");
const ytdl = require("ytdl-core"); // For playing music on YT
var fs = require("fs");
 
// Login and authenticate BOT
const riceBot = new Discord.Client();
riceBot.login(token);

// Get list of tracks URLS
let rawData = fs.readFileSync("usertracks.json");
let userTracks = JSON.parse(rawData);

// For denbugging and seeing the list of user tracks and associated users
function getUserTracks()
{
	for (var i = 0; i < userTracks.userTracks.length; i++)
	{
		console.log(userTracks.userTracks[i].id);
	}
}

// Get list of users who do not have welcome messages, such as bots
let rawData2 = fs.readFileSync("noIntroUsers.json");

// Listener and logging
riceBot.once("ready", () =>
{
    console.log("RiceBot is ready!");
});
riceBot.once("reconnecting", () =>
{
    console.log("RiceBot is reconnecting...");
});
riceBot.once("disconnect", () =>
{
    console.log("Ricebot is disconnected!!");
});
riceBot.once("error", () =>
{
    console.log("ERROR!!!");
});

// Read commands from user messages
riceBot.on("message", async message =>
{
    if (message.author.bot) return; // ignore message if it's from the bot
    if (!message.content.startsWith(prefix)) return; 
}); // assign message

// Whenever a state is changed
riceBot.on("voiceStateUpdate", (oldState, newState) =>
{
    if ((newState.channelID !== null) && (oldState.channelID === null))
    {
        console.log(oldState.member.displayName + " joined the chat");
        var voiceChannel = newState.channel; // channel user just joined

        // Bot joins channel
        voiceChannel.join().then(connection =>
        {
            let streamOptions = {seek: 0, volume: 1};
            let defaultGreeting = ytdl('https://www.youtube.com/watch?v=i8a3gjt_Ar0');
            let stream = defaultGreeting;

            for (var i = 0; i < userTracks.userTracks.length; i++)
            {
                // Check to see if the user is in the list of users
                if (newState.member.id == userTracks.userTracks[i].id)
                {
                    stream = ytdl(userTracks.userTracks[i].track);
                }
            }
            const dispatcher = connection.play(stream, streamOptions);

            // Timeout for discord bot
            setTimeout(function()
            {
                dispatcher.on("finish", () => 
                {
                    console.log("Welcome for %s ended", newState.member.displayName);
                    voiceChannel.leave();
                });
                voiceChannel.leave();
            }, 10000);
        }).catch(err => console.log(err));
    }
    else if ((newState.channelID === null) && (oldState.channelID !== null))
    {
        console.log(newState.member.displayName + " left the chat");
    }
});

// COMMANDS FOR RICEBOTS
riceBot.on("message", message =>
{
    // Format the command to lowercase
	if (!message.content.startsWith(prefix) || message.author.bot) return;
	const args = message.content.slice(prefix.length).split(' ');
	const command = args.shift().toLowerCase();

    // Set track command. allows user to manually set the track for their intro
	if (command === 'settrack')
	{
		let stream = args[0];
        // Search through list of user tracks
		for (var i = 0; i < userTracks.userTracks.length; i++)
		{
			if (userTracks.userTracks[i].id === message.author.id)
			{
				userTracks.userTracks[i].track = stream;
				console.log(`${userTracks.userTracks[i].id}: ${userTracks.userTracks[i].track}`);
			}
		}
		fs.writeFile('usertracks.json', JSON.stringify(userTracks, null, 4), function (err, result)
		{
			if(err) console.log('error', err);
		});
	}
});
