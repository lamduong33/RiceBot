module.exports = {
  name: "ready",
  once: true,
  execute(client) {
    console.log(`RiceBot is ready! Logged in as ${client.user.tag}`);
  },
};
