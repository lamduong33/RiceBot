// Import the discord.js module
const Discord = require('discord.js');
const ytdl = require('ytdl-core');
var fs = require("fs"); //JSON

// Parse JSON file for user tracks
let rawdata = fs.readFileSync('usertracks.json');
let userTracks = JSON.parse(rawdata);
let prefix="!";
// Create an instance of a Discord client
const bot = new Discord.Client();

let users = new Array();

// Parse JSON Data
function getData()
{
	for (var i = 0; i < userTracks.userTracks.length; i++)
	{
		console.log(userTracks.userTracks[i].id);
	}
}

// Get the users
function getUsers()
{
	let guilds = bot.guilds.array();
	for (let i = 0; i < guilds.length; i++)
	{
        bot.guilds.get(guilds[i].id).fetchMembers().then(r =>
		{
			r.members.array().forEach(r =>
			{
				let username = `${r.user.username}`;
				let userID = `${r.user.id}`;
				users.push(username);
				console.log(`${username}: ${userID}`);
			});
		});
	}
}

// Get users for recording purposes
getUsers();

bot.login('NTY0MTYzODI4MDQxMTIxNzk1.XXGTTw.laL-lM2N_VQ_e58w5Q61Y_Rk4nI')
bot.on('ready', () =>
{
	console.log('RiceBot is ready!');
});

// Detecting users joining or leaving channel for "WELCOME TO THE RICE FIELDS"
bot.on('voiceStateUpdate', (oldMember, newMember) => 
{
	getUsers();
    let newUserChannel = newMember.voiceChannel;
    let oldUserChannel = oldMember.voiceChannel;

    const streamOptions = { seek: 0, volume: 1 };
    
    //User Joins a voice channel
    if(oldUserChannel === undefined && newUserChannel !== undefined)
    {
        // If user is not Groovy
        if (newMember.id != 234395307759108106)
        {
            const streamOptions = { seek: 0, volume: 1 };
            console.log(newMember.displayName + ' joined the chat');
            var voiceChannel = newMember.voiceChannel;
    
            voiceChannel.join().then(connection =>
            {
                // Play stream of the default "Welcome to the rice fields"
        		let stream = ytdl('https://www.youtube.com/watch?v=i8a3gjt_Ar0');
        		for (var i = 0; i < userTracks.userTracks.length; i++)
        		{
                    // Check to see if the user is in the list of users
        			if (newMember.id == userTracks.userTracks[i].id)
        			{
        				stream = ytdl(userTracks.userTracks[i].track);
        			}
        		}
                const dispatcher = connection.playStream(stream, streamOptions);
                setTimeout(function()
                {
                    dispatcher.on("end", () => 
                    {
                        console.log("end");
                        voiceChannel.leave();
                    });
                }, 1000);
        
            }).catch(err => console.log(err));
        }
    }
})

bot.on('message', message =>
{
	if (!message.content.startsWith(prefix) || message.author.bot) return;
	const args = message.content.slice(prefix.length).split(' ');
	const command = args.shift().toLowerCase();

	if (command === 'settrack')
	{
		var m = JSON.parse(fs.readFileSync('usertracks.json').toString());	
		let stream = args[0];
		
		for (var i = 0; i < userTracks.userTracks.length; i++)
		{
			if (userTracks.userTracks[i].id === message.author.id)
			{
				userTracks.userTracks[i].track = stream;
				console.log(`${userTracks.userTracks[i].id}: ${userTracks.userTracks[i].track}`);
				message.reply(`You've set your track to ${stream}`);
			}
		}
		fs.writeFile('usertracks.json', JSON.stringify(userTracks, null, 4), function (err,result)
		{
			if(err) console.log('error', err);
		});
	}
});
