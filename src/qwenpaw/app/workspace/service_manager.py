# -*- coding: utf-8 -*-
"""Service management system for Workspace components.

Provides unified registration, lifecycle management, and dependency handling
for all workspace services (MemoryManager, ChatManager, etc.).
"""
from __future__ import annotations

import asyncio
import logging
import time
from dataclasses import dataclass, field
from enum import StrEnum
from functools import partial
from typing import (
    TYPE_CHECKING,
    Any,
    Awaitable,
    Callable,
    Dict,
    List,
    Optional,
    Set,
    Union,
)

if TYPE_CHECKING:
    from .workspace import Workspace

logger = logging.getLogger(__name__)


class ServiceDependencyError(RuntimeError):
    """Raised when the registered service dependency graph is invalid."""


class ServiceStartStatus(StrEnum):
    """Terminal result of one service during a startup attempt."""

    STARTED = "started"
    REUSED = "reused"
    SKIPPED_OPTIONAL = "skipped_optional"
    FAILED = "failed"


@dataclass(frozen=True)
class ServiceStartResult:
    """Structured startup result for one service node."""

    status: ServiceStartStatus
    error: Optional[BaseException] = None
    blocked_by: tuple[str, ...] = ()


@dataclass
class ServiceDescriptor:
    """Descriptor for a workspace service component.

    Defines metadata and lifecycle hooks for a service that can be
    managed by ServiceManager.

    Attributes:
        name: Unique service identifier (e.g., 'memory_manager')
        service_class: Class to instantiate (e.g., MemoryManager)
        init_args: Callable that returns init kwargs for the service
        post_init: Optional hook called after creation (for setup logic)
        start_method: Name of method to call after creation (e.g., 'start')
        stop_method: Name of method to call during shutdown (e.g., 'stop')
        reusable: Whether this service can be reused across reloads
        reload_func: Optional hook called when reusable service is reused
        dependencies: List of service names that must start before this one
        after: Order-only dependencies. Registered services named here start
            first, but their absence or failure does not block this service.
        priority: Startup priority (lower = earlier, reversed for shutdown)
        concurrent_init: Whether this can be initialized concurrently
        optional: If True, a failure during start logs but does not abort
            the workspace; the service is simply absent.
    """

    name: str
    service_class: Optional[Union[type, Callable[["Workspace"], type]]] = None
    init_args: Optional[Callable[[Workspace], dict]] = None
    post_init: Optional[
        Union[
            Callable[[Workspace, Any], None],
            Callable[[Workspace, Any], Awaitable[Any]],
        ]
    ] = None
    start_method: Optional[str] = None
    stop_method: Optional[str] = None
    reusable: bool = False
    reload_func: Optional[
        Union[
            Callable[[Workspace, Any], None],
            Callable[[Workspace, Any], Awaitable[Any]],
        ]
    ] = None
    dependencies: List[str] = field(default_factory=list)
    after: List[str] = field(default_factory=list)
    priority: int = 100
    concurrent_init: bool = True
    optional: bool = False


class ServiceManager:
    """Unified manager for workspace service components.

    Handles registration, lifecycle (start/stop), dependency resolution,
    and component reuse during reload.
    """

    def __init__(self, workspace: Workspace):
        """Initialize service manager.

        Args:
            workspace: The Workspace instance that owns these services
        """
        self.workspace = workspace
        self.services: Dict[str, Any] = {}
        self.descriptors: Dict[str, ServiceDescriptor] = {}
        self.reused_services: Set[str] = set()
        self.last_start_results: Dict[str, ServiceStartResult] = {}

    def register(self, descriptor: ServiceDescriptor) -> None:
        """Register a service descriptor.

        Args:
            descriptor: Service descriptor to register
        """
        if descriptor.name in self.descriptors:
            logger.warning(
                f"Service '{descriptor.name}' already registered, "
                f"overwriting",
            )
        self.descriptors[descriptor.name] = descriptor
        logger.debug(f"Registered service: {descriptor.name}")

    async def set_reusable(self, name: str, instance: Any) -> None:
        """Mark a service instance as reused from previous workspace.

        Must be called before start_all(). If the service descriptor has a
        reload_func, it will be called with the workspace and instance.

        Args:
            name: Service name
            instance: Service instance to reuse
        """
        if name not in self.descriptors:
            logger.warning(
                f"Unknown service '{name}', cannot mark as reusable",
            )
            return

        descriptor = self.descriptors[name]
        if not descriptor.reusable:
            logger.warning(
                f"Service '{name}' is not marked as reusable "
                f"in its descriptor",
            )
            return

        self.services[name] = instance
        self.reused_services.add(name)
        logger.debug(f"Marked service '{name}' as reused")

        # Trigger reload_func if provided
        if descriptor.reload_func is not None:
            try:
                result = descriptor.reload_func(self.workspace, instance)
                if asyncio.iscoroutine(result):
                    await result
                logger.debug(f"Called reload_func for service '{name}'")
            except Exception as e:
                logger.warning(
                    f"Error calling reload_func for service '{name}': {e}",
                )

    def get_reusable_services(self) -> Dict[str, Any]:
        """Get all reusable service instances for transfer to new workspace.

        Returns:
            Dict mapping service names to instances
        """
        reusable = {}
        for name, descriptor in self.descriptors.items():
            if descriptor.reusable and name in self.services:
                reusable[name] = self.services[name]
        return reusable

    @staticmethod
    def _group_by_priority(
        descriptors: List[ServiceDescriptor],
    ) -> Dict[int, List[ServiceDescriptor]]:
        """Group service descriptors by priority.

        Returns:
            Dict mapping priority to list of descriptors
        """
        groups: Dict[int, List[ServiceDescriptor]] = {}
        for descriptor in descriptors:
            if descriptor.priority not in groups:
                groups[descriptor.priority] = []
            groups[descriptor.priority].append(descriptor)
        return groups

    def startup_layers(self) -> List[List[str]]:
        """Validate the graph and return deterministic Kahn layers.

        This method only inspects descriptors.  In particular, it does not
        construct services or invoke lifecycle hooks, so ``start_all`` can
        reject an invalid graph before producing any runtime effects.
        """
        missing = [
            (descriptor.name, dependency)
            for descriptor in self.descriptors.values()
            for dependency in descriptor.dependencies
            if dependency not in self.descriptors
        ]
        if missing:
            details = ", ".join(
                f"'{consumer}' requires '{dependency}'"
                for consumer, dependency in missing
            )
            raise ServiceDependencyError(
                f"Missing required service dependencies: {details}",
            )

        adjacency: Dict[str, List[str]] = {
            name: [] for name in self.descriptors
        }
        indegree = {name: 0 for name in self.descriptors}
        for descriptor in self.descriptors.values():
            predecessors = dict.fromkeys(
                [
                    *descriptor.dependencies,
                    *(
                        name
                        for name in descriptor.after
                        if name in self.descriptors
                    ),
                ],
            )
            for predecessor in predecessors:
                adjacency[predecessor].append(descriptor.name)
                indegree[descriptor.name] += 1

        current = [
            name for name in self.descriptors if indegree[name] == 0
        ]
        layers: List[List[str]] = []
        visited = 0
        while current:
            layers.append(current)
            visited += len(current)
            next_layer: List[str] = []
            for name in current:
                for dependent in adjacency[name]:
                    indegree[dependent] -= 1
                    if indegree[dependent] == 0:
                        next_layer.append(dependent)
            current = next_layer

        if visited != len(self.descriptors):
            cycle = self._find_cycle(adjacency)
            path = " -> ".join(cycle) if cycle else "unknown"
            raise ServiceDependencyError(
                f"Service dependency cycle detected: {path}",
            )
        return layers

    @staticmethod
    def _find_cycle(adjacency: Dict[str, List[str]]) -> List[str]:
        """Return one deterministic cycle path, including its repeated root."""
        state = {name: 0 for name in adjacency}
        stack: List[str] = []
        positions: Dict[str, int] = {}

        def visit(name: str) -> Optional[List[str]]:
            state[name] = 1
            positions[name] = len(stack)
            stack.append(name)
            for dependent in adjacency[name]:
                if state[dependent] == 0:
                    cycle = visit(dependent)
                    if cycle:
                        return cycle
                elif state[dependent] == 1:
                    return [*stack[positions[dependent] :], dependent]
            stack.pop()
            positions.pop(name)
            state[name] = 2
            return None

        for name in adjacency:
            if state[name] == 0:
                cycle = visit(name)
                if cycle:
                    return cycle
        return []

    async def start_all(self) -> Dict[str, ServiceStartResult]:
        """Start all registered services in dependency order.

        The dependency graph is validated before any service is constructed.
        Each topological layer is split into priority groups; services within
        a group preserve the existing ``concurrent_init`` behavior.
        """
        t0 = time.perf_counter()
        logger.debug(
            f"Starting {len(self.descriptors)} services "
            f"({len(self.reused_services)} reused)",
        )

        layers = self.startup_layers()
        self.last_start_results = {}

        for layer in layers:
            descriptors = [self.descriptors[name] for name in layer]
            priority_groups = self._group_by_priority(descriptors)
            for priority in sorted(priority_groups):
                group = priority_groups[priority]

                ready: List[ServiceDescriptor] = []
                for descriptor in group:
                    blocked_by = tuple(
                        dependency
                        for dependency in descriptor.dependencies
                        if self.last_start_results[dependency].status
                        not in {
                            ServiceStartStatus.STARTED,
                            ServiceStartStatus.REUSED,
                        }
                    )
                    if not blocked_by:
                        ready.append(descriptor)
                        continue
                    error = RuntimeError(
                        f"Service '{descriptor.name}' blocked by failed "
                        f"required dependencies: {', '.join(blocked_by)}",
                    )
                    status = (
                        ServiceStartStatus.SKIPPED_OPTIONAL
                        if descriptor.optional
                        else ServiceStartStatus.FAILED
                    )
                    self.last_start_results[descriptor.name] = (
                        ServiceStartResult(
                            status=status,
                            error=error,
                            blocked_by=blocked_by,
                        )
                    )
                    if descriptor.optional:
                        logger.warning(str(error))
                    else:
                        raise error

                concurrent = [d for d in ready if d.concurrent_init]
                sequential = [d for d in ready if not d.concurrent_init]

                if concurrent:
                    results = await asyncio.gather(
                        *[
                            self._start_service(descriptor)
                            for descriptor in concurrent
                        ],
                    )
                    for descriptor, result in zip(concurrent, results):
                        self.last_start_results[descriptor.name] = result
                    self._raise_first_required_failure(concurrent)

                for descriptor in sequential:
                    result = await self._start_service(descriptor)
                    self.last_start_results[descriptor.name] = result
                    self._raise_first_required_failure([descriptor])

                # Yield between priority groups so the event loop can serve
                # requests during background startup.
                await asyncio.sleep(0)

        elapsed = time.perf_counter() - t0
        logger.debug(
            f"All services started for {self.workspace.agent_id} "
            f"in {elapsed:.3f}s",
        )
        return dict(self.last_start_results)

    def _raise_first_required_failure(
        self,
        descriptors: List[ServiceDescriptor],
    ) -> None:
        for descriptor in descriptors:
            result = self.last_start_results[descriptor.name]
            if result.status == ServiceStartStatus.FAILED:
                assert result.error is not None
                raise result.error

    async def _start_service(
        self,
        descriptor: ServiceDescriptor,
    ) -> ServiceStartResult:
        """Start a single service.

        Args:
            descriptor: Service descriptor
        """
        t0 = time.perf_counter()
        name = descriptor.name
        is_reused = name in self.reused_services

        if is_reused:
            logger.info(
                f"Reusing service '{name}' for {self.workspace.agent_id}",
            )

        try:
            service = await self._get_or_create_service(
                descriptor,
                is_reused,
            )
            service = await self._run_post_init(descriptor, service, name)
            await self._run_start_method(descriptor, service, is_reused)

            elapsed = time.perf_counter() - t0
            if elapsed > 0.05:
                logger.debug(
                    f"Service '{name}' ready for "
                    f"{self.workspace.agent_id} ({elapsed:.3f}s)",
                )

            status = (
                ServiceStartStatus.REUSED
                if is_reused
                else ServiceStartStatus.STARTED
            )
            return ServiceStartResult(status=status)

        except Exception as e:
            if descriptor.optional:
                logger.warning(
                    f"Optional service '{name}' failed to start for "
                    f"{self.workspace.agent_id} (continuing without it): {e}",
                )
                self.services.pop(name, None)
                return ServiceStartResult(
                    status=ServiceStartStatus.SKIPPED_OPTIONAL,
                    error=e,
                )
            logger.exception(
                f"Failed to start service '{name}' "
                f"for {self.workspace.agent_id}: {e}",
            )
            return ServiceStartResult(
                status=ServiceStartStatus.FAILED,
                error=e,
            )

    async def _get_or_create_service(
        self,
        descriptor: ServiceDescriptor,
        is_reused: bool,
    ) -> Any:
        """Get existing or create new service instance.

        Synchronous constructors are offloaded to a thread pool via
        ``asyncio.to_thread`` so they do not block the event loop
        (important during background startup when the server is already
        accepting HTTP requests).

        Args:
            descriptor: Service descriptor
            is_reused: Whether service is being reused

        Returns:
            Service instance or None
        """
        if is_reused:
            return self.services.get(descriptor.name)

        logger.debug(f"Creating service '{descriptor.name}'...")

        service_factory = descriptor.service_class
        if service_factory is None:
            return None

        def _resolve_constructor() -> tuple[Any, dict]:
            """Resolve class and arguments without holding the event loop."""
            # service_class may be a callable that resolves to the actual
            # class.  Some resolvers import optional backends or read config.
            if not isinstance(service_factory, type):
                service_cls = service_factory(self.workspace)
            else:
                service_cls = service_factory

            init_kwargs = {}
            if descriptor.init_args:
                init_kwargs = descriptor.init_args(self.workspace)
            return service_cls, init_kwargs

        service_cls, init_kwargs = await asyncio.to_thread(
            _resolve_constructor,
        )

        # Offload synchronous constructor to thread pool to avoid blocking
        # the event loop during background startup.
        service = await asyncio.to_thread(
            partial(service_cls, **init_kwargs),
        )
        self.services[descriptor.name] = service
        return service

    async def _run_post_init(
        self,
        descriptor: ServiceDescriptor,
        service: Any,
        name: str,
    ) -> Any:
        """Run post_init hook and capture returned service.

        Args:
            descriptor: Service descriptor
            service: Current service instance (may be None)
            name: Service name

        Returns:
            Final service instance
        """
        if not descriptor.post_init:
            return service

        await asyncio.sleep(0)
        result = descriptor.post_init(self.workspace, service)
        if asyncio.iscoroutine(result):
            result = await result

        # Capture service from post_init return value or self.services
        if result is not None:
            service = result
            # Ensure it's registered in services dict
            if name not in self.services:
                self.services[name] = service
        elif service is None:
            # post_init might have registered service in self.services
            service = self.services.get(name)

        return service

    async def _run_start_method(
        self,
        descriptor: ServiceDescriptor,
        service: Any,
        is_reused: bool,
    ) -> None:
        """Run start method on service if applicable.

        Synchronous start methods are offloaded to a thread pool.

        Args:
            descriptor: Service descriptor
            service: Service instance
            is_reused: Whether service is being reused
        """
        if is_reused or not descriptor.start_method or not service:
            return

        await asyncio.sleep(0)
        start_fn = getattr(service, descriptor.start_method)
        if asyncio.iscoroutinefunction(start_fn):
            await start_fn()
        else:
            await asyncio.to_thread(start_fn)

        logger.debug(
            f"Service '{descriptor.name}' started for "
            f"{self.workspace.agent_id}",
        )

    async def stop_all(self, final: bool = False) -> None:
        """Stop all services in reverse topological layers.

        Args:
            final: If True, stop ALL services including reusable ones.
                   If False (default), skip reusable services (for reload).

        Reused services are skipped. Errors are logged but don't stop
        the shutdown process.
        """
        logger.debug(
            f"Stopping {len(self.services)} services "
            f"({len(self.reused_services)} reused, final={final})",
        )

        layers = self.startup_layers()

        for layer in reversed(layers):
            descriptors = [self.descriptors[name] for name in layer]

            # Stop all services in this priority group concurrently
            results = await asyncio.gather(
                *[
                    self._stop_service(desc, final=final)
                    for desc in descriptors
                ],
                return_exceptions=True,
            )

            # Log any exceptions that occurred
            for desc, result in zip(descriptors, results):
                if isinstance(result, Exception):
                    logger.warning(
                        f"Error stopping service '{desc.name}': {result}",
                    )

    async def _stop_service(
        self,
        descriptor: ServiceDescriptor,
        final: bool = False,
    ) -> None:
        """Stop a single service.

        Args:
            descriptor: Service descriptor
            final: If True, stop service even if reusable.
                   If False, skip reusable services (for reload).
        """
        name = descriptor.name

        # Skip reusable services UNLESS this is final shutdown
        # (may be transferred to new instance during reload)
        if descriptor.reusable and not final:
            logger.debug(
                f"Skipped stopping reusable service '{name}' "
                f"for {self.workspace.agent_id} (will be reused)",
            )
            return

        # Skip services that were reused from previous instance UNLESS final
        # (they don't belong to this instance, but must be stopped on final)
        if name in self.reused_services and not final:
            logger.debug(
                f"Skipped stopping reused service '{name}' "
                f"(from previous instance) for {self.workspace.agent_id}",
            )
            return

        service = self.services.get(name)
        if not service:
            return

        try:
            if descriptor.stop_method:
                stop_fn = getattr(service, descriptor.stop_method, None)
                if stop_fn:
                    if asyncio.iscoroutinefunction(stop_fn):
                        await stop_fn()
                    else:
                        stop_fn()
                    logger.debug(
                        f"Service '{name}' stopped "
                        f"for {self.workspace.agent_id}",
                    )
        except Exception as e:
            logger.warning(
                f"Error stopping service '{name}' "
                f"for {self.workspace.agent_id}: {e}",
            )
            raise
