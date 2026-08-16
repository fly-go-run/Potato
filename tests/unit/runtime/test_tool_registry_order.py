# -*- coding: utf-8 -*-
from potato.runtime.tool_registry import ToolDescriptor, ToolRegistry


def test_filter_returns_tools_in_name_order():
    registry = ToolRegistry()
    for name in ("zeta", "alpha", "mid"):
        registry.register(
            ToolDescriptor(name=name, func=lambda: None, description=name),
        )

    names = [desc.name for desc in registry.filter()]
    assert names == ["alpha", "mid", "zeta"]
