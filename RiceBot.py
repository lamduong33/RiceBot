import discord

ricebot = discord.Client()

@client.event
async def on_ready():
    print("We have logged in as {0.user}".format(ricebot))

@client.event
async def on_message(message):
    if message.author == ricebot.user:
        return
    if message.content.startswith("$hello"):
        await message.channel.send("Hello!")

ricebot.run("NTY0MTYzODI4MDQxMTIxNzk1.XKj4gg.5JuYZiftXpKaU3SNcUZEKEvbIpM")
