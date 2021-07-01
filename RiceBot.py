import os
import discord
from discord.ext import commands
import logging
from discord.player import AudioSource

from dotenv import load_dotenv  # --require=dotenv/config
import youtube_dl
import asyncio

# ===================================SETUP===================================
load_dotenv()
DISCORD_TOKEN = os.getenv("DISCORD_TOKEN")
intents = discord.Intents().all()
command_prefix = "!"
intents.members = True
# Logging/debugging purposes
logging.basicConfig(level=logging.INFO)
defaultURL = "https://www.youtube.com/watch?v=HKtaHdA6IFc"  # WELCOME

# ===================================RICEBOT===================================
class RiceBot(commands.Bot):
    def __init__(self, t_prefix: str, t_intents: discord.Intents):
        super().__init__(command_prefix=t_prefix, intents=t_intents)

    async def on_ready(self):
        print("Logged in as {0} ({0.id})".format(ricebot.user))
        print("------")

    async def on_voice_state_update(
        self,
        member: discord.Member,
        before: discord.VoiceState,
        after: discord.VoiceState,
    ):
        """Once the user joins the channel, play a track to them"""
        if before.channel == None and after.channel != None:
            print("{} joined!".format(member.display_name))

            # Joins the channel
            await after.channel.connect()

            # Play the music
            player = await YTDLSource.from_url(defaultURL, loop=self.loop)
            duration = player.data["duration"]
            member.guild.voice_client.play(
                player, after=lambda e: print("Player error: %s" % e) if e else None
            )
            # Disconnect
            await asyncio.sleep(duration)
            await member.guild.voice_client.disconnect()
        elif after.channel == None:
            print("{} left!".format(member.display_name))


ricebot = RiceBot(command_prefix, intents)


# =====================================YTDL======================================
youtube_dl.utils.bug_reports_message = lambda: ""
ytdl_format_options = {
    "format": "bestaudio/best",
    "outtmpl": "%(extractor)s-%(id)s-%(title)s.%(ext)s",
    "restrictfilenames": True,
    "noplaylist": True,
    "nocheckcertificate": True,
    "ignoreerrors": False,
    "logtostderr": False,
    "quiet": True,
    "no_warnings": True,
    "default_search": "auto",
    "source_address": "0.0.0.0",  # bind to ipv4 since ipv6 addresses cause issues sometimes
}
ffmpeg_options = {"options": "-vn"}
ytdl = youtube_dl.YoutubeDL(ytdl_format_options)


class YTDLSource(discord.PCMVolumeTransformer):
    """Class for the YTDL Source, such as a video"""

    def __init__(self, source, *, data, volume=0.5):
        super().__init__(source, volume)
        self.data = data
        self.title = data.get("title")
        self.url = ""

    @classmethod
    async def from_url(cls, url, *, loop=None, stream=False):
        loop = loop or asyncio.get_event_loop()
        data = await loop.run_in_executor(
            None, lambda: ytdl.extract_info(url, download=not stream)
        )

        if "entries" in data:
            # take first item from a playlist
            data = data["entries"][0]

        filename = data["url"] if stream else ytdl.prepare_filename(data)
        return cls(discord.FFmpegPCMAudio(filename, **ffmpeg_options), data=data)


# ======================================MUSIC======================================
class Music(commands.Cog):
    """Music cog taken from DiscordPY documentation"""

    def __init__(self, bot):
        self.bot = bot

    @commands.command()
    async def join(self, ctx, *, channel: discord.VoiceChannel):
        """Joins a voice channel"""

        if ctx.voice_client is not None:
            return await ctx.voice_client.move_to(channel)

        await channel.connect()

    @commands.command()
    async def play(self, ctx, *, query):
        """Plays a file from the local filesystem"""

        source = discord.PCMVolumeTransformer(discord.FFmpegPCMAudio(query))
        ctx.voice_client.play(
            source, after=lambda e: print("Player error: %s" % e) if e else None
        )

        await ctx.send("Now playing: {}".format(query))

    @commands.command()
    async def yt(self, ctx, *, url):
        """Plays from a url (almost anything youtube_dl supports)"""

        async with ctx.typing():
            player = await YTDLSource.from_url(url, loop=self.bot.loop)
            ctx.voice_client.play(
                player, after=lambda e: print("Player error: %s" % e) if e else None
            )

        await ctx.send("Now playing: {}".format(player.title))

    @commands.command()
    async def stream(self, ctx, *, url):
        """Streams from a url (same as yt, but doesn't predownload)"""

        async with ctx.typing():
            player = await YTDLSource.from_url(url, loop=self.bot.loop, stream=True)
            ctx.voice_client.play(
                player, after=lambda e: print("Player error: %s" % e) if e else None
            )

        await ctx.send("Now playing: {}".format(player.title))

    @commands.command()
    async def volume(self, ctx, volume: int):
        """Changes the player's volume"""

        if ctx.voice_client is None:
            return await ctx.send("Not connected to a voice channel.")

        ctx.voice_client.source.volume = volume / 100
        await ctx.send("Changed volume to {}%".format(volume))

    @commands.command()
    async def stop(self, ctx):
        """Stops and disconnects the bot from voice"""

        await ctx.voice_client.disconnect()

    @play.before_invoke
    @yt.before_invoke
    @stream.before_invoke
    async def ensure_voice(self, ctx):
        if ctx.voice_client is None:
            if ctx.author.voice:
                await ctx.author.voice.channel.connect()
            else:
                await ctx.send("You are not connected to a voice channel.")
                raise commands.CommandError("Author not connected to a voice channel.")
        elif ctx.voice_client.is_playing():
            ctx.voice_client.stop()


# ======================================MAIN======================================

ricebot.add_cog(Music(ricebot))
if __name__ == "__main__":
    ricebot.run(DISCORD_TOKEN)
    ricebot.run("NTY0MTYzODI4MDQxMTIxNzk1.XKj4gg.5JuYZiftXpKaU3SNcUZEKEvbIpM")
