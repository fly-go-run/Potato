# Base images — override with --build-arg for environments without ACR access.
ARG NODE_IMAGE=agentscope-registry.ap-southeast-1.cr.aliyuncs.com/agentscope/node:slim
ARG UV_IMAGE=agentscope-registry.ap-southeast-1.cr.aliyuncs.com/agentscope/uv:latest

# -----------------------------------------------------------------------------
# Stage 1: build the default web UI (dist is not committed in repo).
# -----------------------------------------------------------------------------
FROM ${NODE_IMAGE} AS web-app-builder
WORKDIR /app
COPY app /app/app
RUN cd /app/app && npm ci --include=dev && npm run build

# Alias for uv binary so COPY --from can reference a build-arg image.
FROM ${UV_IMAGE} AS uv-src

# -----------------------------------------------------------------------------
# Stage 2: runtime image with Python, Chromium, and app.
# -----------------------------------------------------------------------------
FROM ${NODE_IMAGE}

# ENV variables
ENV NODE_ENV=production
ENV WORKSPACE_DIR=/app
ENV POTATO_WORKING_DIR=/app/working
ENV POTATO_SECRET_DIR=/app/working.secret
ENV POTATO_BACKUP_DIR=/app/working.backups

# Channel filtering: use POTATO_DISABLED_CHANNELS (exclusion, recommended)
# or POTATO_ENABLED_CHANNELS (whitelist). Override at runtime with -e.
ARG POTATO_DISABLED_CHANNELS="imessage"
ENV POTATO_DISABLED_CHANNELS=${POTATO_DISABLED_CHANNELS}
ARG POTATO_ENABLED_CHANNELS=""
ENV POTATO_ENABLED_CHANNELS=${POTATO_ENABLED_CHANNELS}

ARG DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --fix-missing \
    curl  \
    python3  \
    python3-pip  \
    python3-venv \
    build-essential  \
    libssl-dev  \
    git  \
    supervisor  \
    vim  \
    gettext-base \
    xfce4 \
    xfce4-terminal \
    xvfb \
    dbus-x11 \
    fonts-wqy-zenhei \
    fonts-wqy-microhei \
  && rm -rf /var/lib/apt/lists/* \
  && apt-get clean


RUN apt-get update && apt-get install -y --fix-missing \
    chromium \
    chromium-sandbox \
    libx11-xcb1 \
    libxcomposite1 \
    libxdamage1 \
    libxext6 \
    libxfixes3 \
    libxi6 \
    libxtst6 \
    libnss3 \
    libglib2.0-0 \
    libdrm2 \
    libgbm1 \
    libasound2 \
    fonts-liberation \
    libu2f-udev \
  && rm -rf /var/lib/apt/lists/* \
  && apt-get clean


RUN sed -i 's/^CHROMIUM_FLAGS=""/CHROMIUM_FLAGS="--no-sandbox"/' /usr/bin/chromium

# Playwright: use system Chromium (already installed above).
ENV PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH=/usr/bin/chromium
# Avoid Playwright downloading its own browser when executable_path is used.
ENV PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1
# Indicate running in container (used e.g. for Chromium --no-sandbox).
ENV POTATO_RUNNING_IN_CONTAINER=1

WORKDIR ${WORKSPACE_DIR}

RUN python3 -m venv venv
ENV PATH="/app/venv/bin:$PATH"

COPY pyproject.toml setup.py README.md ./
COPY src ./src
COPY website/public/docs/ ./src/potato/docs/
# Stage the default web UI in the historical potato/console package-data
# location used by the backend's /console route.
COPY --from=web-app-builder /app/app/dist/ ./src/potato/console/
# Speed up Python package installation with uv.
COPY --from=uv-src /uv /bin/uv
RUN uv pip install --no-cache-dir . && rm -rf ./build


# Potato app port (default 8088). Override at runtime with -e POTATO_PORT=3000.
ENV POTATO_PORT=8088

COPY deploy/config/supervisord.conf.template /etc/supervisor/conf.d/supervisord.conf.template
COPY --chmod=755 deploy/entrypoint.sh /entrypoint.sh

EXPOSE 8088

CMD ["/entrypoint.sh"]
