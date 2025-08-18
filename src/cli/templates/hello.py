from nonebot import on_command
from nonebot.adapters import Message
from nonebot.params import CommandArg
from nonebot.plugin import PluginMetadata

__plugin_meta__ = PluginMetadata(
    name="Hello Plugin",
    description="A simple hello plugin",
    usage="Send 'hello' to get a greeting",
)

hello = on_command("hello", aliases={"hi"}, priority=10, block=True)

@hello.handle()
async def hello_handler(args: Message = CommandArg()):
    msg = args.extract_plain_text()
    if msg:
        await hello.finish(f"Hello, {msg}!")
    else:
        await hello.finish("Hello, World!")