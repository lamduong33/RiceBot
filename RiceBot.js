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

    const streamOptions = { seek: 0, volume: 1 };
    
    //User Joins a voice channel
    if(oldUserChannel === undefined && newUserChannel !== undefined)
    {
        const streamOptions = { seek: 0, volume: 1 };
        console.log(newMember.displayName + ' joined the chat');
        var voiceChannel = newMember.voiceChannel;
        voiceChannel.join().then(connection =>
        {
            let stream = ytdl('https://www.youtube.com/watch?v=i8a3gjt_Ar0');
            if (newMember.displayName == "Llamanator")
            {
                stream = ytdl('https://www.youtube.com/watch?v=3WAOxKOmR90');
            }
            if (newMember.displayName == "This Guy Fucks")
            {
                stream = ytdl('https://www.youtube.com/watch?v=dPSi6w5QnP0');
            }
            if (newMember.displayName == "Lootenant Dan")
            {
                stream = ytdl('https://youtu.be/8X_Ot0k4XJc');
            }
            const dispatcher = connection.playStream(stream, streamOptions);
            
            setTimeout(function(){
                dispatcher.on("end", () => 
                {
                    console.log("end");
                    voiceChannel.leave();
                });
            }, 1000);

        }).catch(err => console.log(err));
    }
})