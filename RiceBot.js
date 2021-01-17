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
            let stream = ytdl('https://www.youtube.com/watch?v=i8a3gjt_Ar0');
            const dispatcher = connection.play(stream, streamOptions);

            // Timeout for discord bot
            setTimeout(function()
            {
                dispatcher.on("finish", () => 
                {
                    console.log("Welcome for %s ended", newState.member.displayName);
                    voiceChannel.leave();
                });
            }, 1000);
        }).catch(err => console.log(err));
    }
    else if ((newState.channelID === null) && (oldState.channelID !== null))
    {
        console.log(newState.member.displayName + " left the chat");
    }
});
