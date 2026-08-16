# Scripts

Run from **repo root**.

## Build wheel (with the default web app)

```bash
bash scripts/wheel_build.sh
```

- Builds the default web app (`app/`), copies `app/dist` to `src/potato/console/` (the historical package-data location for `/console`), then builds the wheel. Output: `dist/*.whl`.

## Build website

```bash
bash scripts/website_build.sh
```

- Installs dependencies (pnpm or npm) and runs the Vite build. Output: `website/dist/`.

## Build Docker image

```bash
bash scripts/docker_build.sh [IMAGE_TAG] [EXTRA_ARGS...]
```

- Default tag: `potato:latest`. Uses `deploy/Dockerfile` (multi-stage: builds the web app then Python app).
- Example: `bash scripts/docker_build.sh myreg/potato:v1 --no-cache`.

## Run Test

```bash
# Run all tests
python scripts/run_tests.py

# Run all unit tests
python scripts/run_tests.py -u

# Run unit tests for a specific module
python scripts/run_tests.py -u providers

# Run integration tests
python scripts/run_tests.py -i

# Run all tests and generate a coverage report
python scripts/run_tests.py -a -c

# Run tests in parallel (requires pytest-xdist)
python scripts/run_tests.py -p

# Show help
python scripts/run_tests.py -h
```
