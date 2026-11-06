"""Account e sincronizzazione Nexus: un processo, SQLite, stessa origine della UI."""
from __future__ import annotations

from contextlib import contextmanager
from datetime import datetime, timezone
from hashlib import scrypt, sha256
import hmac
import json
import os
from pathlib import Path
import re
import secrets
import sqlite3
import threading
import time
from urllib.parse import urlsplit

from fastapi import FastAPI, HTTPException, Request, Response
from fastapi.responses import FileResponse, JSONResponse
from fastapi.exceptions import RequestValidationError
from fastapi.staticfiles import StaticFiles
from pydantic import BaseModel, ConfigDict, Field, field_validator

MAX_BODY = 2_000_000
SESSION_AGE = 7 * 24 * 60 * 60
COOKIE = "nexus_session"


class StrictModel(BaseModel):
    model_config = ConfigDict(extra="forbid", strict=True)

    @field_validator("*", mode="before")
    @classmethod
    def valid_utf8(cls, value):
        if isinstance(value, str):
            value.encode("utf-8")
        return value


def clean_text(value: str) -> str:
    value.encode("utf-8")
    if any(ord(char) < 32 for char in value):
        raise ValueError("Caratteri di controllo non consentiti")
    value = value.strip()
    if not value:
        raise ValueError("Testo vuoto o caratteri di controllo")
    return value


class Step(StrictModel):
    id: str = Field(min_length=1, max_length=80, pattern=r"^[a-zA-Z0-9_-]+$")
    action: str = Field(min_length=1, max_length=120)
    createdAt: str = Field(max_length=40)

    _text = field_validator("action")(clean_text)

    @field_validator("createdAt")
    @classmethod
    def timestamp(cls, value: str) -> str:
        if not re.fullmatch(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d{1,6})?(?:Z|[+-]\d\d:\d\d)", value):
            raise ValueError("Data ISO con fuso orario richiesta")
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
        if not 1970 <= parsed.year <= 9999:
            raise ValueError("Data non valida")
        return value


class Goal(StrictModel):
    id: str = Field(min_length=1, max_length=80, pattern=r"^[a-zA-Z0-9_-]+$")
    title: str = Field(min_length=1, max_length=160)
    nextStep: str = Field(max_length=240)
    area: str = Field(pattern=r"^(personal|stability)$")
    done: bool

    _text = field_validator("title")(clean_text)

    @field_validator("nextStep")
    @classmethod
    def next_step(cls, value: str) -> str:
        if any(ord(char) < 32 and char != "\n" for char in value):
            raise ValueError("Caratteri di controllo non consentiti")
        return value.strip()


class Journey(StrictModel):
    version: int = Field(ge=2, le=2)
    paused: bool
    steps: list[Step] = Field(max_length=10000)
    goals: list[Goal] = Field(max_length=100)
    interests: list[str] = Field(max_length=50)

    @field_validator("steps", "goals")
    @classmethod
    def unique_ids(cls, values):
        if len({item.id for item in values}) != len(values):
            raise ValueError("Identificatori duplicati")
        return values

    @field_validator("interests")
    @classmethod
    def interest_names(cls, values: list[str]) -> list[str]:
        values = [clean_text(value) for value in values]
        if any(len(value) > 60 for value in values) or len({value.lower() for value in values}) != len(values):
            raise ValueError("Interessi duplicati o troppo lunghi")
        return values


def empty_journey() -> dict:
    return {"version": 2, "paused": False, "steps": [], "goals": [], "interests": []}


class Credentials(StrictModel):
    username: str = Field(min_length=3, max_length=40, pattern=r"^[a-zA-Z0-9._-]+$")
    password: str = Field(min_length=12, max_length=128)


class Registration(Credentials):
    displayName: str = Field(min_length=1, max_length=60)
    invitation: str = Field(max_length=200)

    _text = field_validator("displayName")(clean_text)


class Recovery(Credentials):
    recoveryCode: str = Field(min_length=1, max_length=200)


class PasswordChange(StrictModel):
    currentPassword: str = Field(min_length=1, max_length=128)
    password: str = Field(min_length=12, max_length=128)


class Deletion(StrictModel):
    password: str = Field(min_length=1, max_length=128)


class Update(StrictModel):
    revision: int = Field(ge=0)
    journey: Journey


def password_hash(password: str, salt: bytes | None = None) -> str:
    salt = salt or secrets.token_bytes(16)
    digest = scrypt(password.encode(), salt=salt, n=32768, r=8, p=1, maxmem=64 * 1024 * 1024)
    return salt.hex() + ":" + digest.hex()


def verify_password(password: str, stored: str) -> bool:
    return hmac.compare_digest(password_hash(password, bytes.fromhex(stored.split(":")[0])), stored)


def token_hash(token: str) -> str:
    return sha256(token.encode()).hexdigest()


class Settings:
    def __init__(self, *, database: Path | None = None, origin: str | None = None, invitation: str | None = None):
        self.database = database or Path(os.environ.get("NEXUS_DATABASE", "server/data/nexus.sqlite3"))
        self.origin = (origin or os.environ.get("NEXUS_ORIGIN", "http://127.0.0.1:5173")).rstrip("/")
        parsed = urlsplit(self.origin)
        if parsed.path or parsed.query or parsed.fragment or parsed.username or not parsed.hostname:
            raise ValueError("NEXUS_ORIGIN deve contenere solo schema, host e porta")
        if parsed.scheme != "https" and not (parsed.scheme == "http" and parsed.hostname in ("localhost", "127.0.0.1", "::1")):
            raise ValueError("Accesso remoto: configurare HTTPS in NEXUS_ORIGIN")
        self.secure = parsed.scheme == "https"
        self.invitation = invitation if invitation is not None else os.environ.get("NEXUS_INVITATION", "")
        if self.invitation and len(self.invitation) < 16:
            raise ValueError("NEXUS_INVITATION deve avere almeno 16 caratteri")


class Database:
    def __init__(self, path: Path):
        self.path = path
        path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        with self.connect() as db:
            db.executescript("""
                PRAGMA journal_mode=WAL;
                CREATE TABLE IF NOT EXISTS users (
                    id TEXT PRIMARY KEY, username TEXT UNIQUE NOT NULL, display_name TEXT NOT NULL,
                    password TEXT NOT NULL, recovery TEXT NOT NULL,
                    journey TEXT NOT NULL, revision INTEGER NOT NULL DEFAULT 0,
                    updated_at TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS sessions (
                    token TEXT PRIMARY KEY, user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                    csrf TEXT NOT NULL, expires INTEGER NOT NULL
                );
            """)
        os.chmod(path, 0o600)

    @contextmanager
    def connect(self):
        db = sqlite3.connect(self.path, timeout=15)
        db.row_factory = sqlite3.Row
        db.execute("PRAGMA foreign_keys=ON")
        db.execute("PRAGMA secure_delete=ON")
        try:
            with db:
                yield db
        finally:
            db.close()


def create_app(settings: Settings | None = None) -> FastAPI:
    settings = settings or Settings()
    database = Database(settings.database)
    app = FastAPI(docs_url=None, redoc_url=None, openapi_url=None)
    app.state.database = database
    # Limitazione in memoria: un solo processo Uvicorn, come nel deployment fornito.
    attempts: dict[str, list[float]] = {}
    attempts_lock = threading.Lock()
    dummy_hash = password_hash(secrets.token_urlsafe(32))

    @app.exception_handler(RequestValidationError)
    async def validation_error(_request, _error):
        # Gli errori standard includono l'input: potrebbe essere una password.
        return JSONResponse({"detail": "Dati non validi. Controlla formato e lunghezza dei campi."}, status_code=422)

    def throttle(request: Request, username: str = ""):
        now = time.monotonic()
        host = request.client.host if request.client else "unknown"
        with attempts_lock:
            for key in list(attempts):
                attempts[key] = [stamp for stamp in attempts[key] if now - stamp < 900]
                if not attempts[key]:
                    del attempts[key]
            keys = [("ip:" + host, 30), ("user:" + username.lower(), 12)]
            if any(len(attempts.get(key, [])) >= limit for key, limit in keys):
                raise HTTPException(429, "Troppi tentativi. Riprova tra 15 minuti.", headers={"Retry-After": "900"})
            for key, _ in keys:
                attempts.setdefault(key, []).append(now)

    @app.middleware("http")
    async def security(request: Request, call_next):
        if request.method not in ("GET", "HEAD"):
            if request.headers.get("origin") != settings.origin:
                return JSONResponse({"detail": "Origine non autorizzata."}, status_code=403)
            if request.headers.get("content-type", "").split(";")[0] != "application/json":
                return JSONResponse({"detail": "Richiesta JSON necessaria."}, status_code=415)
            body = bytearray()
            async for chunk in request.stream():
                body.extend(chunk)
                if len(body) > MAX_BODY:
                    return JSONResponse({"detail": "File troppo grande (massimo 2 MB)."}, status_code=413)
            request._body = bytes(body)
        response = await call_next(request)
        response.headers["Cache-Control"] = "no-store"
        response.headers["X-Content-Type-Options"] = "nosniff"
        response.headers["Referrer-Policy"] = "no-referrer"
        response.headers["X-Frame-Options"] = "DENY"
        response.headers["Content-Security-Policy"] = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'"
        return response

    def current(request: Request):
        token = request.cookies.get(COOKIE, "")
        with database.connect() as db:
            row = db.execute("SELECT users.*, sessions.csrf, sessions.token FROM sessions JOIN users ON users.id=sessions.user_id WHERE sessions.token=? AND expires>?", (token_hash(token), int(time.time()))).fetchone()
        if not row:
            raise HTTPException(401, "Accedi per sincronizzare il tuo percorso.")
        if request.method not in ("GET", "HEAD") and not hmac.compare_digest(request.headers.get("x-nexus-csrf", "").encode(), row["csrf"].encode()):
            raise HTTPException(403, "Sessione non valida. Ricarica la pagina.")
        return row

    def session(response: Response, user_id: str) -> str:
        token, csrf = secrets.token_urlsafe(32), secrets.token_urlsafe(32)
        with database.connect() as db:
            db.execute("DELETE FROM sessions WHERE expires<=?", (int(time.time()),))
            db.execute("INSERT INTO sessions VALUES (?,?,?,?)", (token_hash(token), user_id, csrf, int(time.time()) + SESSION_AGE))
        response.set_cookie(COOKIE, token, httponly=True, secure=settings.secure, samesite="strict", max_age=SESSION_AGE, path="/")
        return csrf

    def public_user(row):
        return {"id": row["id"], "username": row["username"], "displayName": row["display_name"]}

    @app.get("/api/health")
    def health():
        with database.connect() as db:
            db.execute("SELECT 1").fetchone()
        return {"status": "ok"}

    @app.get("/api/config")
    def config():
        return {"registrationEnabled": bool(settings.invitation)}

    @app.post("/api/register", status_code=201)
    def register(data: Registration, request: Request, response: Response):
        throttle(request, data.username)
        if not settings.invitation or not hmac.compare_digest(data.invitation.encode(), settings.invitation.encode()):
            raise HTTPException(403, "Codice di invito non valido o registrazioni chiuse.")
        user_id, recovery = secrets.token_hex(16), secrets.token_urlsafe(32)
        now = datetime.now(timezone.utc).isoformat()
        try:
            with database.connect() as db:
                db.execute("INSERT INTO users VALUES (?,?,?,?,?,?,0,?)", (user_id, data.username.lower(), data.displayName, password_hash(data.password), token_hash(recovery), json.dumps(empty_journey()), now))
        except sqlite3.IntegrityError:
            raise HTTPException(409, "Nome utente già utilizzato.") from None
        csrf = session(response, user_id)
        return {"user": {"id": user_id, "username": data.username.lower(), "displayName": data.displayName}, "csrf": csrf, "recoveryCode": recovery}

    @app.post("/api/login")
    def login(data: Credentials, request: Request, response: Response):
        throttle(request, data.username)
        with database.connect() as db:
            row = db.execute("SELECT * FROM users WHERE username=?", (data.username.lower(),)).fetchone()
        valid = verify_password(data.password, row["password"] if row else dummy_hash)
        if not row or not valid:
            raise HTTPException(401, "Nome utente o password non corretti.")
        csrf = session(response, row["id"])
        return {"user": public_user(row), "csrf": csrf}

    @app.post("/api/recover")
    def recover(data: Recovery, request: Request, response: Response):
        throttle(request, data.username)
        next_recovery = secrets.token_urlsafe(32)
        with database.connect() as db:
            row = db.execute("SELECT * FROM users WHERE username=?", (data.username.lower(),)).fetchone()
            if not row or not hmac.compare_digest(row["recovery"], token_hash(data.recoveryCode.strip())):
                raise HTTPException(401, "Nome utente o codice di recupero non corretti.")
            # CAS: il codice non può essere riutilizzato neppure da richieste concorrenti.
            changed = db.execute("UPDATE users SET password=?, recovery=? WHERE id=? AND recovery=?", (password_hash(data.password), token_hash(next_recovery), row["id"], row["recovery"])).rowcount
            if not changed:
                raise HTTPException(409, "Codice già utilizzato. Usa il nuovo codice.")
            db.execute("DELETE FROM sessions WHERE user_id=?", (row["id"],))
        return {"user": public_user(row), "csrf": session(response, row["id"]), "recoveryCode": next_recovery}

    @app.get("/api/session")
    def get_session(request: Request):
        row = current(request)
        return {"user": public_user(row), "csrf": row["csrf"]}

    @app.post("/api/logout")
    def logout(request: Request, response: Response):
        row = current(request)
        with database.connect() as db:
            db.execute("DELETE FROM sessions WHERE token=?", (row["token"],))
        response.delete_cookie(COOKIE, path="/", secure=settings.secure, httponly=True, samesite="strict")
        return {"ok": True}

    @app.post("/api/password")
    def change_password(data: PasswordChange, request: Request, response: Response):
        row = current(request)
        throttle(request, row["username"])
        if not verify_password(data.currentPassword, row["password"]):
            raise HTTPException(401, "Password attuale non corretta.")
        recovery = secrets.token_urlsafe(32)
        with database.connect() as db:
            changed = db.execute("UPDATE users SET password=?, recovery=? WHERE id=? AND password=?", (password_hash(data.password), token_hash(recovery), row["id"], row["password"])).rowcount
            if not changed:
                raise HTTPException(409, "Credenziali aggiornate altrove. Accedi di nuovo.")
            db.execute("DELETE FROM sessions WHERE user_id=?", (row["id"],))
        return {"csrf": session(response, row["id"]), "recoveryCode": recovery}

    @app.get("/api/journey")
    def get_journey(request: Request):
        row = current(request)
        return {"journey": json.loads(row["journey"]), "revision": row["revision"], "updatedAt": row["updated_at"]}

    @app.put("/api/journey")
    def update_journey(data: Update, request: Request):
        row = current(request)
        now = datetime.now(timezone.utc).isoformat()
        with database.connect() as db:
            changed = db.execute("UPDATE users SET journey=?, revision=revision+1, updated_at=? WHERE id=? AND revision=?", (data.journey.model_dump_json(), now, row["id"], data.revision)).rowcount
            if not changed:
                raise HTTPException(409, "Il percorso è cambiato su un altro dispositivo. Scegli quale versione conservare.")
        return {"revision": data.revision + 1, "updatedAt": now}

    @app.post("/api/account/delete")
    def delete_account(data: Deletion, request: Request, response: Response):
        row = current(request)
        throttle(request, row["username"])
        if not verify_password(data.password, row["password"]):
            raise HTTPException(401, "Password non corretta.")
        with database.connect() as db:
            changed = db.execute("DELETE FROM users WHERE id=? AND password=?", (row["id"], row["password"])).rowcount
            if not changed:
                raise HTTPException(409, "Credenziali aggiornate altrove. Accedi di nuovo.")
        response.delete_cookie(COOKIE, path="/", secure=settings.secure, httponly=True, samesite="strict")
        return {"ok": True}

    dist = Path(__file__).resolve().parents[1] / "dist"
    if dist.is_dir():
        if (dist / "assets").is_dir():
            app.mount("/assets", StaticFiles(directory=dist / "assets"), name="assets")

        @app.get("/")
        def index():
            return FileResponse(dist / "index.html")

        @app.get("/favicon.ico")
        def favicon():
            path = dist / "favicon.ico"
            if not path.is_file():
                return Response(status_code=204)
            return FileResponse(path)

    return app
