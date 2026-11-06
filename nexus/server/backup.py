"""Backup SQLite consistente anche mentre il servizio è in uso."""
import argparse
from contextlib import closing
import os
from pathlib import Path
import sqlite3


def backup(source: Path, destination: Path):
    if not source.is_file():
        raise ValueError("Database sorgente non trovato")
    # Non sovrascrive mai un backup esistente, e nasce con permessi privati.
    descriptor = os.open(destination, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    os.close(descriptor)
    try:
        with closing(sqlite3.connect(source.resolve().as_uri() + "?mode=ro", uri=True)) as src:
            with closing(sqlite3.connect(destination)) as dst:
                src.backup(dst)
                if dst.execute("PRAGMA integrity_check").fetchone()[0] != "ok":
                    raise ValueError("Controllo di integrità non superato")
    except Exception:
        destination.unlink(missing_ok=True)
        raise


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    backup(args.source, args.destination)
    print("Backup creato e controllo di integrità superato.")
