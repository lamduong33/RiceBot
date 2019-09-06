// Import the discord.js module
const Discord = require('discord.js');
//const ytdl = require('ytdl-core');


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

    //User Joins a voice channel
    if(oldUserChannel === undefined && newUserChannel !== undefined)
    {
        console.log(newMember + ' joined the chat');
        var voiceChannel = newMember.voiceChannel;
        voiceChannel.join().then(connection =>
        {
            //const dispatcher = connection.playConvertedStream(ytdl('https://www.youtube.com/watch?v=i8a3gjt_Ar0', { filter: 'audioonly' }));
            const dispatcher = connection.playFile('./audio/WELCOME TO THE RICE FIELDS MOTHERFUCKER.mp3');
            
            dispatcher.on("end", () => 
            {
                console.log('Successfully played soundbyte');
            });
            setTimeout(function(){
                voiceChannel.leave();
            },5000);
        }).catch(err => console.log(err));
    }
})