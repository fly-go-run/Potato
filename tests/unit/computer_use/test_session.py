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
