from __future__ import annotations


class AutomationError(RuntimeError):
    """Base error for auto page capture."""


class AutomationEnvironmentError(AutomationError):
    """Raised when Windows automation cannot run in the current environment."""


class AutomationFallbackRequired(AutomationError):
    """Raised when auto page capture should fall back to manual capture."""
