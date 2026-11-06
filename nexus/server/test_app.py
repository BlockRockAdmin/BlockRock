from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import tempfile
import time
import unittest

from fastapi.testclient import TestClient

from server.app import COOKIE, Settings, create_app, empty_journey
from server.backup import backup

ORIGIN = "http://127.0.0.1:5173"
INVITATION = "test-only-invitation-code"
PASSWORD = "test-only-password-123"


class NexusApiTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.settings = Settings(database=Path(self.directory.name) / "nexus.db", origin=ORIGIN, invitation=INVITATION)
        self.app = create_app(self.settings)
        self.client = TestClient(self.app, headers={"Origin": ORIGIN})
        self.addCleanup(self.client.close)

    def register(self, username="utente-test", client=None):
        client = client or self.client
        response = client.post("/api/register", json={"username": username, "password": PASSWORD, "displayName": "Utente Test", "invitation": INVITATION})
        self.assertEqual(response.status_code, 201, response.text)
        client.headers["X-Nexus-CSRF"] = response.json()["csrf"]
        return response.json()

    def login(self, client, password=PASSWORD):
        response = client.post("/api/login", json={"username": "utente-test", "password": password})
        self.assertEqual(response.status_code, 200, response.text)
        client.headers["X-Nexus-CSRF"] = response.json()["csrf"]
        return response

    def test_registration_cookie_and_secret_hashes(self):
        result = self.register()
        response = self.client.get("/api/session")
        self.assertEqual(response.json()["user"]["username"], "utente-test")
        self.assertNotIn("recovery", response.text)
        with self.app.state.database.connect() as db:
            user = dict(db.execute("SELECT * FROM users").fetchone())
            session = dict(db.execute("SELECT * FROM sessions").fetchone())
        self.assertNotEqual(user["password"], PASSWORD)
        self.assertNotEqual(user["recovery"], result["recoveryCode"])
        self.assertNotEqual(session["token"], self.client.cookies[COOKIE])
        cookie = self.client.cookies[COOKIE]
        self.assertGreater(len(cookie), 40)
        self.assertEqual(self.settings.database.stat().st_mode & 0o777, 0o600)

    def test_login_logout_and_expiry(self):
        response = self.client.post("/api/register", json={"username": "utente-test", "password": PASSWORD, "displayName": "A", "invitation": INVITATION})
        self.assertIn("HttpOnly", response.headers["set-cookie"])
        self.assertIn("SameSite=strict", response.headers["set-cookie"])
        self.client.cookies.clear()
        self.assertEqual(self.client.post("/api/login", json={"username": "utente-test", "password": PASSWORD + "wrong"}).status_code, 401)
        self.login(self.client)
        self.assertEqual(self.client.post("/api/logout", json={}).status_code, 200)
        self.assertEqual(self.client.get("/api/journey").status_code, 401)
        self.login(self.client)
        with self.app.state.database.connect() as db:
            db.execute("UPDATE sessions SET expires=?", (int(time.time()) - 1,))
        self.assertEqual(self.client.get("/api/session").status_code, 401)

    def test_data_isolation_between_accounts(self):
        self.register()
        journey = empty_journey()
        journey["interests"] = ["Solo mio"]
        self.assertEqual(self.client.put("/api/journey", json={"revision": 0, "journey": journey}).status_code, 200)
        other = TestClient(self.app, headers={"Origin": ORIGIN})
        self.addCleanup(other.close)
        self.register("secondo", other)
        self.assertEqual(other.get("/api/journey").json()["journey"], empty_journey())
        self.assertEqual(self.client.get("/api/journey").json()["journey"]["interests"], ["Solo mio"])
        self.assertEqual(other.get("/api/journey?user_id=utente-test").json()["journey"], empty_journey())

    def test_second_device_and_conflict_no_data_loss(self):
        self.register()
        other = TestClient(self.app, headers={"Origin": ORIGIN})
        self.addCleanup(other.close)
        self.login(other)
        journey = empty_journey()
        journey["steps"] = [{"id": "step-1", "action": "Sono uscito", "createdAt": "2026-11-05T12:00:00.000Z"}]
        self.assertEqual(self.client.put("/api/journey", json={"revision": 0, "journey": journey}).status_code, 200)
        self.assertEqual(other.put("/api/journey", json={"revision": 0, "journey": empty_journey()}).status_code, 409)
        self.assertEqual(other.get("/api/journey").json()["journey"], journey)

    def test_concurrent_compare_and_swap(self):
        self.register()
        def update(interest):
            client = TestClient(self.app, headers=dict(self.client.headers), cookies=dict(self.client.cookies))
            try:
                journey = empty_journey(); journey["interests"] = [interest]
                return client.put("/api/journey", json={"revision": 0, "journey": journey}).status_code
            finally:
                client.close()
        with ThreadPoolExecutor(max_workers=2) as executor:
            statuses = list(executor.map(update, ["Uno", "Due"]))
        self.assertEqual(sorted(statuses), [200, 409])
        self.assertEqual(self.client.get("/api/journey").json()["revision"], 1)

    def test_restart_and_full_database_backup(self):
        self.register()
        journey = empty_journey(); journey["paused"] = True
        journey["goals"] = [{"id": "goal-1", "title": "Riserva", "nextStep": "Riguardare le spese", "area": "stability", "done": False}]
        self.client.put("/api/journey", json={"revision": 0, "journey": journey})
        target = Path(self.directory.name) / "backup.db"
        backup(self.settings.database, target)
        self.assertEqual(target.stat().st_mode & 0o777, 0o600)
        with self.assertRaises(FileExistsError):
            backup(self.settings.database, target)
        restored = TestClient(create_app(Settings(database=target, origin=ORIGIN, invitation="")), headers={"Origin": ORIGIN})
        self.addCleanup(restored.close)
        self.login(restored)
        self.assertEqual(restored.get("/api/journey").json()["journey"], journey)

    def test_recovery_rotates_code_and_revokes_sessions(self):
        original = self.register()
        other = TestClient(self.app, headers={"Origin": ORIGIN})
        self.addCleanup(other.close)
        response = other.post("/api/recover", json={"username": "utente-test", "password": "next-password-12345", "recoveryCode": original["recoveryCode"]})
        self.assertEqual(response.status_code, 200)
        self.assertNotEqual(response.json()["recoveryCode"], original["recoveryCode"])
        self.assertEqual(self.client.get("/api/session").status_code, 401)
        self.assertEqual(other.post("/api/recover", json={"username": "utente-test", "password": PASSWORD, "recoveryCode": original["recoveryCode"]}).status_code, 401)
        self.client.cookies.clear()
        self.login(self.client, "next-password-12345")

    def test_password_change_and_account_deletion(self):
        self.register()
        other = TestClient(self.app, headers={"Origin": ORIGIN})
        self.addCleanup(other.close)
        self.login(other)
        response = self.client.post("/api/password", json={"currentPassword": PASSWORD, "password": "changed-password-123"})
        self.assertEqual(response.status_code, 200)
        self.client.headers["X-Nexus-CSRF"] = response.json()["csrf"]
        self.assertEqual(other.get("/api/session").status_code, 401)
        self.assertEqual(self.client.post("/api/account/delete", json={"password": PASSWORD}).status_code, 401)
        self.assertEqual(self.client.post("/api/account/delete", json={"password": "changed-password-123"}).status_code, 200)
        with self.app.state.database.connect() as db:
            self.assertEqual(db.execute("SELECT COUNT(*) FROM users").fetchone()[0], 0)
            self.assertEqual(db.execute("SELECT COUNT(*) FROM sessions").fetchone()[0], 0)

    def test_origin_csrf_content_type_and_size(self):
        self.register()
        self.assertEqual(self.client.put("/api/journey", headers={"Origin": "https://evil.invalid"}, json={"revision": 0, "journey": empty_journey()}).status_code, 403)
        self.assertEqual(self.client.put("/api/journey", headers={"X-Nexus-CSRF": "wrong"}, json={"revision": 0, "journey": empty_journey()}).status_code, 403)
        self.assertEqual(self.client.post("/api/login", content="username=test").status_code, 415)
        self.assertEqual(self.client.post("/api/login", content='"' + 'x' * 2_000_000 + '"', headers={"Content-Type": "application/json"}).status_code, 413)
        self.assertEqual(self.client.get("/api/journey").json()["revision"], 0)

    def test_validation_does_not_leak_input_or_change_state(self):
        self.register()
        cases = []
        bad = empty_journey(); bad["steps"] = [{"id": "x", "action": "Gesto", "createdAt": "2026-02-30T12:00:00Z"}]; cases.append(bad)
        bad = empty_journey(); bad["interests"] = ["Musica", "musica"]; cases.append(bad)
        bad = empty_journey(); bad["goals"] = [{"id": "x", "title": "Goal", "nextStep": "", "area": "trading", "done": False}]; cases.append(bad)
        bad = empty_journey(); bad["paused"] = "false"; cases.append(bad)
        bad = empty_journey(); bad["steps"] = [{"id": "x", "action": "Gesto", "createdAt": "2026-11-05T12:00:00Z"}] * 2; cases.append(bad)
        for journey in cases:
            with self.subTest(journey=journey):
                response = self.client.put("/api/journey", json={"revision": 0, "journey": journey})
                self.assertEqual(response.status_code, 422)
                self.assertNotIn('"input"', response.text)
        response = self.client.post("/api/login", json={"username": "a", "password": "SECRET"})
        self.assertNotIn("SECRET", response.text)
        self.assertEqual(self.client.get("/api/journey").json()["revision"], 0)

    def test_invitation_closed_signup_and_duplicate_names(self):
        self.assertEqual(self.client.post("/api/register", json={"username": "test", "password": PASSWORD, "displayName": "Test", "invitation": "wrong"}).status_code, 403)
        self.register()
        self.assertEqual(self.client.post("/api/register", json={"username": "UTENTE-TEST", "password": PASSWORD, "displayName": "Test", "invitation": INVITATION}).status_code, 409)
        closed = TestClient(create_app(Settings(database=Path(self.directory.name) / "closed.db", origin=ORIGIN, invitation="")), headers={"Origin": ORIGIN})
        self.addCleanup(closed.close)
        self.assertFalse(closed.get("/api/config").json()["registrationEnabled"])
        self.assertEqual(closed.post("/api/register", json={"username": "test", "password": PASSWORD, "displayName": "Test", "invitation": ""}).status_code, 403)

    def test_rate_limit(self):
        for _ in range(12):
            self.assertEqual(self.client.post("/api/login", json={"username": "missing", "password": PASSWORD}).status_code, 401)
        response = self.client.post("/api/login", json={"username": "missing", "password": PASSWORD})
        self.assertEqual(response.status_code, 429)
        self.assertIn("Retry-After", response.headers)

    def test_unicode_input_is_rejected_safely_or_roundtrips(self):
        response = self.client.post("/api/register", json={"username": "test", "password": PASSWORD, "displayName": "Test", "invitation": "invito-non-valido-🌱"})
        self.assertEqual(response.status_code, 403)
        response = self.client.post("/api/login", content='{"username":"test","password":"a-password-\\ud800"}', headers={"Content-Type": "application/json"})
        self.assertEqual(response.status_code, 422)
        self.register()
        journey = empty_journey()
        journey["steps"] = [{"id": "unicode-1", "action": "🌱" * 100, "createdAt": "2026-11-05T12:00:00Z"}]
        journey["interests"] = ["Straße", "STRASSE"]
        journey["goals"] = [{"id": "multiline", "title": "Due passi", "nextStep": "Prima una cosa\nPoi scelgo il resto", "area": "personal", "done": False}]
        self.assertEqual(self.client.put("/api/journey", json={"revision": 0, "journey": journey}).status_code, 200)
        self.assertEqual(self.client.get("/api/journey").json()["journey"], journey)

    def test_https_configuration_and_private_headers(self):
        with self.assertRaises(ValueError):
            Settings(origin="http://192.168.8.1:8788")
        with self.assertRaises(ValueError):
            Settings(origin="https://host.invalid/path")
        https = TestClient(create_app(Settings(database=Path(self.directory.name) / "secure.db", origin="https://nexus.private.invalid", invitation=INVITATION)), base_url="https://nexus.private.invalid", headers={"Origin": "https://nexus.private.invalid"})
        self.addCleanup(https.close)
        response = https.post("/api/register", json={"username": "test", "password": PASSWORD, "displayName": "Test", "invitation": INVITATION})
        self.assertIn("Secure", response.headers["set-cookie"])
        self.assertEqual(response.headers["cache-control"], "no-store")
        self.assertIn("frame-ancestors 'none'", response.headers["content-security-policy"])


if __name__ == "__main__":
    unittest.main()
