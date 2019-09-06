// Import the discord.js module
const Discord = require('discord.js');
const ytdl = require('ytdl-core');


// Create an instance of a Discord client
const bot = new Discord.Client()

bot.login('NTY0MTYzODI4MDQxMTIxNzk1.XXGTTw.laL-lM2N_VQ_e58w5Q61Y_Rk4nI')
bot.on('ready', () => {
    console.log('I am ready!');
});

// Detecting users joining or leaving channel for "WELCOME TO THE RICE FIELDS"
bot.on('voiceStateUpdate', (oldMember, newMember) => 
{
    let newUserChannel = newMember.voiceChannel;
    let oldUserChannel = oldMember.voiceChannel;
    // Play streams using ytdl-core
    const ytdl = require('ytdl-core');
    const streamOptions = { seek: 0, volume: 1 };
    //User Joins a voice channel
    if(oldUserChannel === undefined && newUserChannel !== undefined)
    {
        const streamOptions = { seek: 0, volume: 1 };
        console.log(newMember + ' joined the chat');
        var voiceChannel = newMember.voiceChannel;
        voiceChannel.join().then(connection =>
        {
            const stream = ytdl('https://www.youtube.com/watch?v=i8a3gjt_Ar0', { filter : 'audioonly' });
            const dispatcher = connection.playStream(stream, streamOptions);
        }).catch(err => console.log(err));
    }
})