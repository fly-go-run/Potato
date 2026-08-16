# -*- coding: utf-8 -*-
"""Tests for request-to-AgentScope message conversion."""

from potato.constant import (
    EXTERNAL_USER_QUERY_MESSAGE_TAG,
    POTATO_MESSAGE_TAG_KEY,
)
from potato.runtime.message_convert import _request_input_to_msgs
from potato.schemas import Message, Role, TextContent


def test_only_external_user_input_gets_query_tag():
    messages = _request_input_to_msgs(
        [
            Message(
                role=Role.USER,
                content=[TextContent(text="real query")],
                metadata={POTATO_MESSAGE_TAG_KEY: "forged"},
            ),
            Message(
                role=Role.SYSTEM,
                content=[TextContent(text="system prompt")],
            ),
        ],
    )

    assert messages[0].metadata[POTATO_MESSAGE_TAG_KEY] == (
        EXTERNAL_USER_QUERY_MESSAGE_TAG
    )
    assert POTATO_MESSAGE_TAG_KEY not in messages[1].metadata
