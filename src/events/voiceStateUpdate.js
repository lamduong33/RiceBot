// This file handles the detection of user leaving and entering the voice chat.
const ytdl = require("ytdl-core"); // For playing music on YT
const { joinVoiceChannel, VoiceConnectionStatus, createAudioPlayer, createAudioResource, StreamType } = require('@discordjs/voice');
const streamOptions = { filter: "audioonly", seek: 0, volume: 1 };


// The default message
const welcomeToTheRiceFields = "https://www.youtube.com/watch?v=Y4ket21Tg6w";

module.exports = {
  name: "voiceStateUpdate",
  once: false,
  execute(oldState, newState) {
    // If user has entered the chat
    if (newState.channelId !== null && oldState.channelId === null) {
      console.log(oldState.member.displayName + " joined the chat");
      const botVoice = newState.channel;
      const connection = joinVoiceChannel({
        channelId: botVoice.id,
        guildId: botVoice.guildId,
        adapterCreator: botVoice.guild.voiceAdapterCreator
      })

      // Play the stream here
      const stream = ytdl(welcomeToTheRiceFields, {highWaterMark: 1 << 25,
                                                   filter: "audioonly"});
      const resource = createAudioResource(stream, { inputType: StreamType.Opus })
      const player = createAudioPlayer();
      connection.subscribe(player);
      player.play(resource)
    }
    else if (newState.channelId === null && oldState.channelId !== null) {
      console.log(newState.member.displayName + " left the chat");
    }
  },
};
