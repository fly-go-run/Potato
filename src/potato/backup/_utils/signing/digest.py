# -*- coding: utf-8 -*-
"""Canonical HMAC digest helpers for backup zip archives.

The backup signature is a local trust marker, not a portable certificate.
It binds the archive bytes plus selected metadata to this installation's
private signing key so restore/import can distinguish local backups from
foreign or legacy archives that need explicit user trust.
"""
from __future__ import annotations

import hashlib
import hmac
import json
import zipfile

from ...models import BackupMeta, BackupValidationError
from ..constants import META_FILE
from .key import get_signing_key

SCHEME = "hmac-sha256-v1"
_CHUNK_SIZE = 1024 * 1024

_SIGNED_FIELDS = (
    "id",
    "name",
    "description",
    "created_at",
    "version",
    "scope",
    "agent_count",
    "potato_version",
    "system_info",
    "accepted_via_trust",
)
_LEGACY_SIGNED_FIELDS = tuple(
    "qwenpaw_version" if field == "potato_version" else field
    for field in _SIGNED_FIELDS
)
# ``signature`` itself is deliberately excluded to avoid signing a value that
# changes as a result of the signature calculation.
_EXPLICIT_UNSIGNED = frozenset({"signature"})


def _canonical_meta_bytes(
    meta: BackupMeta,
    *,
    legacy_qwenpaw: bool = False,
) -> bytes:
    """Return stable JSON bytes for metadata covered by the signature."""
    raw = meta.model_dump(mode="json")
    fields = _LEGACY_SIGNED_FIELDS if legacy_qwenpaw else _SIGNED_FIELDS
    data = {}
    for field in fields:
        if field == "qwenpaw_version":
            data[field] = raw.get("potato_version")
        else:
            data[field] = raw.get(field)
    return json.dumps(
        data,
        sort_keys=True,
        ensure_ascii=False,
        separators=(",", ":"),
    ).encode("utf-8")


def _feed_zip_entries(mac: hmac.HMAC, zf: zipfile.ZipFile) -> None:
    """Feed every non-meta zip entry into *mac* in deterministic order.

    The small marker and NUL separators define a simple transcript format:
    entry marker, filename, file bytes, and fixed-width content length. That
    framing keeps variable-length fields unambiguous while still streaming
    large files without loading the whole archive into memory.
    """
    for info in sorted(zf.infolist(), key=lambda item: item.filename):
        if info.is_dir() or info.filename == META_FILE:
            continue
        mac.update(b"ENTRY\x00")
        mac.update(info.filename.encode("utf-8"))
        mac.update(b"\x00")
        size = 0
        with zf.open(info) as fp:
            while chunk := fp.read(_CHUNK_SIZE):
                size += len(chunk)
                mac.update(chunk)
        mac.update(b"\x00")
        mac.update(size.to_bytes(8, "big"))
        mac.update(b"\x00")


def _compute_signature(
    zf: zipfile.ZipFile,
    meta: BackupMeta,
    *,
    legacy_qwenpaw: bool = False,
) -> str:
    signing_meta = meta.model_copy(update={"signature": None})
    mac = hmac.new(get_signing_key(), b"", hashlib.sha256)
    mac.update(b"META\x00")
    mac.update(
        _canonical_meta_bytes(
            signing_meta,
            legacy_qwenpaw=legacy_qwenpaw,
        ),
    )
    mac.update(b"\x00")
    _feed_zip_entries(mac, zf)
    return f"{SCHEME}:{mac.hexdigest()}"


def compute_signature(zf: zipfile.ZipFile, meta: BackupMeta) -> str:
    """Compute this installation's Potato-format signature."""
    return _compute_signature(zf, meta)


def verify_signature(zf: zipfile.ZipFile, meta: BackupMeta) -> bool:
    """Return True when *meta.signature* matches this archive locally."""
    if not meta.signature:
        return False
    scheme, sep, _hex = meta.signature.partition(":")
    if sep != ":" or scheme != SCHEME:
        return False
    expected = compute_signature(zf, meta)
    if hmac.compare_digest(meta.signature, expected):
        return True
    # Signatures made by QwenPaw covered the same value under the old JSON
    # field name. Keep those locally-created backups restorable.
    legacy_expected = _compute_signature(zf, meta, legacy_qwenpaw=True)
    return hmac.compare_digest(meta.signature, legacy_expected)


def signature_error(meta: BackupMeta) -> BackupValidationError:
    """Return the stable validation error for an invalid backup signature."""
    if not meta.signature:
        return BackupValidationError(
            "backup_legacy_unsigned",
            "Backup has no local signature and must be explicitly trusted.",
        )
    scheme, sep, _hex = meta.signature.partition(":")
    if sep != ":" or scheme != SCHEME:
        return BackupValidationError(
            "backup_unknown_signature_scheme",
            "Backup signature uses an unknown signing scheme.",
        )
    return BackupValidationError(
        "backup_signature_mismatch",
        "Backup signature does not match this instance.",
    )


def _assert_signed_fields_cover_model() -> None:
    """Guard tests against adding BackupMeta fields without signing policy."""
    covered = set(_SIGNED_FIELDS) | set(_EXPLICIT_UNSIGNED)
    expected = set(BackupMeta.model_fields)
    if covered != expected:
        missing = sorted(expected - covered)
        extra = sorted(covered - expected)
        raise AssertionError(
            "BackupMeta signing field mismatch: "
            f"missing={missing} extra={extra}",
        )
