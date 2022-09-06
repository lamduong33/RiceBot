// This file handles the detection of user leaving and entering the voice chat.

module.exports = {
  name: "voiceStateUpdate",
  once: false,
  execute(oldState, newState) {
    // Check if a new user has joined the channel due to channel's state change.
    if (newState.channelId !== null && oldState.channelId === null) {
        console.log(oldState.member.displayName + " joined the chat");
    }
    else if (newState.channelId === null && oldState.channelId !== null) {
        console.log(newState.member.displayName + " left the chat");
    }
  },
};
