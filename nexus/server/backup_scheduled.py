"""Create a uniquely named, private, consistent local Nexus backup."""
import argparse
from datetime import datetime, timezone
from pathlib import Path
from uuid import uuid4

from server.backup import backup


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    args.directory.mkdir(mode=0o700, parents=True, exist_ok=True)
    if args.directory.is_symlink() or not args.directory.is_dir():
        raise ValueError("La destinazione deve essere una cartella reale")
    # Never overwrite or remove earlier copies, including on repeated runs.
    timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    destination = args.directory / f"nexus-{timestamp}-{uuid4().hex}.sqlite3"
    backup(args.source, destination)
    print(f"Backup verificato: {destination}")


if __name__ == "__main__":
    main()
