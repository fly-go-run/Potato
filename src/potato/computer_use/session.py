# -*- coding: utf-8 -*-
"""In-process observation store. Tokens die; they are never reused blindly."""

from __future__ import annotations

import secrets
import threading
import time
from dataclasses import dataclass, field
from typing import Any, Callable

from .constants import OBSERVATION_TTL_SECONDS
from .errors import ComputerUseError


@dataclass
class Observation:
    observation_id: str
    app: str
    bundle_id: str
    pid: int
    window_id: int
    snapshot_id: str
    session_id: str = ""
    elements: list[dict[str, Any]] = field(default_factory=list)
    created_at: float = field(default_factory=time.time)

    def expired(self, now: float | None = None) -> bool:
        stamp = now if now is not None else time.time()
        return stamp - self.created_at > OBSERVATION_TTL_SECONDS

    def element(self, index: int) -> dict[str, Any]:
        for item in self.elements:
            if int(item.get("element_index", -1)) == index:
                return item
        raise ComputerUseError(
            "STALE_ELEMENT",
            f"element_index {index} is not in observation {self.observation_id}. "
            "Call computer_observe again.",
        )


class ObservationStore:
    def __init__(self) -> None:
        self._lock = threading.Lock()
        self._items: dict[str, Observation] = {}
        self._reaped_session_ids: list[str] = []
        self._reap_notifier: Callable[[], None] | None = None

    def set_reap_notifier(self, notifier: Callable[[], None] | None) -> None:
        """Set a non-blocking callback invoked when sessions need ending."""
        with self._lock:
            self._reap_notifier = notifier

    def put(self, observation: Observation) -> Observation:
        with self._lock:
            self._purge_unlocked()
            stale = [
                key
                for key, item in self._items.items()
                if item.pid == observation.pid
                and item.window_id == observation.window_id
            ]
            for key in stale:
                self._reap_unlocked(self._items.pop(key, None))
            self._items[observation.observation_id] = observation
        return observation

    def drop(self, observation_id: str) -> None:
        with self._lock:
            self._reap_unlocked(
                self._items.pop((observation_id or "").strip(), None),
            )

    def clear(self) -> None:
        with self._lock:
            for item in self._items.values():
                self._reap_unlocked(item)
            self._items.clear()

    def get(self, observation_id: str) -> Observation:
        key = (observation_id or "").strip()
        with self._lock:
            self._purge_unlocked()
            item = self._items.get(key)
        if item is None:
            raise ComputerUseError(
                "STALE_OBSERVATION",
                "That observation expired or does not exist. "
                "Call computer_observe and use the new observation_id.",
            )
        return item

    def take(self, observation_id: str) -> Observation:
        """Atomically consume one observation without reaping its session.

        The caller owns the returned driver session until its action finishes.
        Concurrent callers cannot take the same observation twice.
        """
        key = (observation_id or "").strip()
        with self._lock:
            self._purge_unlocked()
            item = self._items.pop(key, None)
        if item is None:
            raise ComputerUseError(
                "STALE_OBSERVATION",
                "That observation expired or does not exist. "
                "Call computer_observe and use the new observation_id.",
            )
        return item

    def drain_reaped_session_ids(self) -> list[str]:
        """Return sessions discarded by expiry, replacement, drop, or clear."""
        with self._lock:
            session_ids = self._reaped_session_ids
            self._reaped_session_ids = []
        return session_ids

    def _purge_unlocked(self) -> None:
        now = time.time()
        dead = [k for k, v in self._items.items() if v.expired(now)]
        for key in dead:
            self._reap_unlocked(self._items.pop(key, None))

    def _reap_unlocked(self, observation: Observation | None) -> None:
        if observation is not None and observation.session_id:
            self._reaped_session_ids.append(observation.session_id)
            if self._reap_notifier is not None:
                self._reap_notifier()


_STORE = ObservationStore()


def observation_store() -> ObservationStore:
    return _STORE


def new_observation_id() -> str:
    return "obs_" + secrets.token_urlsafe(12)


def observation_session_id(observation_id: str) -> str:
    return f"potato-{observation_id}"
