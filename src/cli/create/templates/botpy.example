#!/usr/bin/env python3
# -*- coding: utf-8 -*-

import nonebot
import tomllib
import importlib
from nonebot import logger

nonebot.init()

driver = nonebot.get_driver()
# 读取 pyproject.toml 中的 adapter 列表
with open("pyproject.toml", "rb") as f:
    pyproject = tomllib.load(f)
    adapters = pyproject["tool"]["nonebot"]["adapters"]
    for adapter in adapters:
        try:
            adapter_module = importlib.import_module(adapter["module_name"])
            driver.register_adapter(adapter_module.Adapter)
        except Exception as e:
            logger.error(f"Failed to register adapter {adapter['name']}: {e}")
            continue

nonebot.load_from_toml("pyproject.toml")


if __name__ == "__main__":
    nonebot.run(app="__mp_main__:app")
