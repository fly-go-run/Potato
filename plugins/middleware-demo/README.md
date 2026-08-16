# Middleware Demo Plugins

Two example plugins demonstrating `PluginApi.register_middleware()` — the
plugin-based mechanism for injecting AgentScope `MiddlewareBase` instances
into the agent's reasoning loop.

## Included Demos

| Plugin | Hook | Behavior |
|--------|------|----------|
| `tracing-middleware` | `on_acting` | Logs every tool call (name, duration) to a file. Conditionally activated only when `POTATO_TRACE` env var is set. |
| `thinking-log-middleware` | `on_reasoning` | Prints model reasoning stream events to stdout (`[THINKING]` for chain-of-thought, `[TEXT]` for text responses). Always active. |

## Installation

These plugins are **not** auto-loaded. Install them explicitly:

```bash
# While Potato is running (hot-load, no restart needed):
potato plugin install plugins/middleware-demo/tracing-middleware
potato plugin install plugins/middleware-demo/thinking-log-middleware

# Or when Potato is stopped (loaded on next start):
potato plugin install plugins/middleware-demo/tracing-middleware
potato plugin install plugins/middleware-demo/thinking-log-middleware
```

## Uninstall

```bash
potato plugin uninstall middleware-demo-tracing
potato plugin uninstall middleware-demo-thinking-log
```

## How It Works

Each plugin registers a **middleware factory** via `api.register_middleware(factory, priority=N)`.

The factory is called once per request during agent assembly:

```python
def my_factory(ctx, agent_config):
    # ctx: HookContext (session_id, agent_id, workspace_dir, ...)
    # agent_config: AgentProfileConfig
    #
    # Return a MiddlewareBase instance to activate, or None to skip.
    return MyMiddleware()
```

The returned middleware wraps the agent's inner reasoning loop using the
standard AgentScope 2.0 onion model (`on_reply`, `on_reasoning`, `on_acting`).

## Priority

Lower priority values run as the outermost layer (execute first on the way
in, last on the way out). The built-in middlewares use implicit ordering;
plugin middlewares append after them sorted by priority.
