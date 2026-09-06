# -*- coding: utf-8 -*-
from __future__ import annotations

import threading
from concurrent.futures import ThreadPoolExecutor

from potato.computer_use.errors import ComputerUseError
from potato.computer_use.session import Observation, ObservationStore


def test_take_is_single_use_under_concurrency() -> None:
    store = ObservationStore()
    store.put(
        Observation(
            observation_id="obs_once",
            session_id="potato-obs_once",
            app="Calculator",
            bundle_id="com.apple.calculator",
            pid=1,
            window_id=2,
            snapshot_id="snap",
        ),
    )
    barrier = threading.Barrier(2)

    def _take() -> str:
        barrier.wait()
        try:
            return store.take("obs_once").observation_id
        except ComputerUseError as exc:
            return exc.code

    with ThreadPoolExecutor(max_workers=2) as executor:
        results = list(executor.map(lambda _index: _take(), range(2)))

    assert sorted(results) == ["STALE_OBSERVATION", "obs_once"]


def test_hold_for_approval_outlives_ttl() -> None:
    from potato.computer_use.constants import OBSERVATION_TTL_SECONDS

    store = ObservationStore()
    obs = Observation(
        observation_id="obs_hold",
        session_id="potato-obs_hold",
        app="Calculator",
        bundle_id="com.apple.calculator",
        pid=1,
        window_id=2,
        snapshot_id="snap",
    )
    store.put(obs)
    base = obs.created_at
    assert obs.expired(now=base + OBSERVATION_TTL_SECONDS + 1)
    store.hold_for_approval("obs_hold", 330.0)
    # Held: survives well past the base TTL, dies after the hold.
    assert not obs.expired(now=base + OBSERVATION_TTL_SECONDS + 1)
    assert not obs.expired(now=base + 320.0)
    assert obs.expired(now=base + 340.0)
    # Holding an unknown id is a no-op, not an error.
    store.hold_for_approval("obs_nope", 330.0)
