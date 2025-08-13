#!/usr/bin/env python3
# -*- coding: utf-8 -*-

import nonebot
import tomllib
import importlib
from nonebot import logger

nonebot.init()

driver = nonebot.get_driver()

with open("pyproject.toml", "rb") as f:
    pyproject = tomllib.load(f)
    tool_nonebot = pyproject["tool"]["nonebot"]
    adapters = tool_nonebot["adapters"]
    for adapter in adapters:
        try:
            adapter_module = importlib.import_module(adapter["module_name"])
            driver.register_adapter(adapter_module.Adapter)
        except Exception as e:
            logger.error(f"Failed to register adapter {adapter['name']}: {e}")
            continue
    nonebot.load_builtin_plugins(*tool_nonebot["builtin_plugins"])

nonebot.load_from_toml("pyproject.toml")

if __name__ == "__main__":
    nonebot.run()
