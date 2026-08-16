# -*- coding: utf-8 -*-
"""Tauri sidecar environment variable helpers.

Keep this dependency-light: the Tauri entry imports it before potato.constant
has read import-time environment variables.
"""

import os

DESKTOP_APP_ENV = "POTATO_DESKTOP_APP"
DESKTOP_CORS_ORIGINS_ENV = "POTATO_CORS_ORIGINS"
DESKTOP_SHUTDOWN_TOKEN_ENV = "POTATO_DESKTOP_SHUTDOWN_TOKEN"
DESKTOP_READY_PREFIX = "POTATO_BACKEND_READY"

DESKTOP_CORS_ORIGINS = (
    "tauri://localhost",
    "https://tauri.localhost",
    "http://tauri.localhost",
    # `tauri dev` serves the bundled app from the Vite port, not /console.
    "http://localhost:5174",
    "http://127.0.0.1:5174",
)


def ensure_desktop_cors_origins() -> None:
    origins = [
        origin.strip()
        for origin in os.environ.get(DESKTOP_CORS_ORIGINS_ENV, "").split(",")
        if origin.strip()
    ]
    for origin in DESKTOP_CORS_ORIGINS:
        if origin not in origins:
            origins.append(origin)
    os.environ[DESKTOP_CORS_ORIGINS_ENV] = ",".join(origins)
