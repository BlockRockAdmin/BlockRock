"""Avvio privato di Nexus con invito persistente e database locale."""
import os
from pathlib import Path
import secrets

import uvicorn

from server.app import Settings, create_app


def prepare_invitation(database: Path) -> Path | None:
    if "NEXUS_INVITATION" in os.environ:
        return None
    database.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    invitation = database.parent / "invitation.txt"
    try:
        descriptor = os.open(invitation, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except FileExistsError:
        pass
    else:
        with os.fdopen(descriptor, "w") as stream:
            stream.write(secrets.token_urlsafe(32) + "\n")
    os.environ["NEXUS_INVITATION"] = invitation.read_text().strip()
    return invitation


def main():
    database = Path(os.environ.get("NEXUS_DATABASE", "server/data/nexus.sqlite3"))
    invitation = prepare_invitation(database)
    origin = os.environ.get("NEXUS_ORIGIN", "http://127.0.0.1:8788")
    settings = Settings(database=database, origin=origin)
    dist = Path(__file__).resolve().parents[1] / "dist" / "index.html"
    if not dist.is_file():
        raise SystemExit("Prima esegui npm run build dalla cartella nexus.")
    if invitation:
        print(f"Codice di invito privato nel file: {invitation.resolve()}", flush=True)
    print(f"Nexus: {origin}", flush=True)
    uvicorn.run(create_app(settings), host="127.0.0.1", port=8788, workers=1, access_log=False, proxy_headers=False)


if __name__ == "__main__":
    main()
