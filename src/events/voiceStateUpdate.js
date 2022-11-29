// This file handles the detection of user leaving and entering the voice chat.
const { joinVoiceChannel, createAudioPlayer, createAudioResource, NoSubscriberBehavior, VoiceConnectionStatus, AudioPlayerStatus } = require('@discordjs/voice');
const play = require('play-dl');
const userTracks = require("../tracks.json");

// The default message
const welcomeToTheRiceFields = "https://www.youtube.com/watch?v=Y4ket21Tg6w";

module.exports = {
  name: "voiceStateUpdate",
  once: false,
  async execute(oldState, newState) {

    // If user has entered the chat
    if (newState.channelId !== null && oldState.channelId === null) {
      const botVoice = newState.channel;
      const connection = joinVoiceChannel({
        channelId: botVoice.id,
        guildId: botVoice.guildId,
        adapterCreator: botVoice.guild.voiceAdapterCreator
      })

      // Audio resource from a video
      let stream = await play.stream(getTrack(newState.member.id));
      let resource = createAudioResource(stream.stream, {
        inputType: stream.type
      })

      // Create player and play the music
      let player = createAudioPlayer({
        behaviors: {
          noSubscriber: NoSubscriberBehavior.Play
        }
      })
      player.play(resource)
      connection.subscribe(player)

      // Leave the channel
      player.on(AudioPlayerStatus.Idle, async (oldState, newState) => {
        try {
          connection.destroy();
        } catch (error) {
          console.error(error);
        }
      });
    }
  },
};

function getTrack(userId) {
  let track = welcomeToTheRiceFields;
  if (userTracks.some(userTrack => userTrack.userId === userId)) {
    track = userTracks.find(userTrack => userTrack.userId === userId).track;
  }
  return track;
}
