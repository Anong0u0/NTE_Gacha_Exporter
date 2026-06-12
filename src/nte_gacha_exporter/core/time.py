from __future__ import annotations

from datetime import datetime, timedelta

DOTNET_EPOCH_TICKS = 621_355_968_000_000_000
TICKS_PER_SECOND = 10_000_000


def dotnet_ticks_to_iso(ticks: int) -> str | None:
    """Convert .NET DateTime ticks to ISO text when the value is plausible."""

    seconds = (ticks - DOTNET_EPOCH_TICKS) / TICKS_PER_SECOND
    if not (1_500_000_000 <= seconds <= 4_102_444_800):
        return None
    dt = datetime(1, 1, 1) + timedelta(microseconds=ticks // 10)
    return dt.isoformat(timespec="microseconds")
