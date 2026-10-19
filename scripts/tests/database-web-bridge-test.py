"""Pure PHP-CGI bridge fixture. Uses temporary stubs, never a daemon or real database.

Run: python scripts/tests/database-web-bridge-test.py
Override PHP_CGI when php-cgi is not next to the local php executable.
"""
import http.cookies
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
php = shutil.which("php")
cgi = os.environ.get("PHP_CGI") or (str(Path(php).with_name("php-cgi.exe" if os.name == "nt" else "php-cgi")) if php else shutil.which("php-cgi"))
if not cgi or not Path(cgi).is_file():
    raise SystemExit("Set PHP_CGI to a local PHP-CGI executable; no service is started.")

with tempfile.TemporaryDirectory(prefix="db-web-") as directory:
    base = Path(directory)
    state, pma, adminer = base / "state", base / "phpmyadmin", base / "adminer"
    for path in [state, pma / "js", adminer]:
        path.mkdir(parents=True, exist_ok=True)
    source = (ROOT / "templates/database-web/entry.php").read_text()
    source = source.replace("/var/lib/panel-db-web", state.as_posix())
    source = source.replace("/usr/share/phpmyadmin", pma.as_posix())
    source = source.replace("/usr/share/adminer", adminer.as_posix())
    entry = base / "entry.php"
    entry.write_text(source)
    (pma / "index.php").write_text("""<?php
// Exercise PHP's real cookie/session lifecycle with phpMyAdmin's HTTP/HTTPS names.
if (getenv('PMA_SESSION_COOKIE')) {
    session_name(getenv('PMA_SESSION_COOKIE'));
    session_start();
    if (empty($_SESSION['fixture-token'])) {
        session_regenerate_id(true);
        $_SESSION['fixture-token'] = bin2hex(random_bytes(16));
        header('Location: ./index.php?route=/&reload=1');
        session_write_close(); exit;
    }
    if ($_SERVER['REQUEST_METHOD'] === 'POST'
        && ($_POST['token'] ?? '') !== $_SESSION['fixture-token']) {
        http_response_code(403); exit('Invalid CSRF');
    }
    echo json_encode(['token'=>$_SESSION['fixture-token'],'cookies'=>array_keys($_COOKIE)]);
    session_write_close(); exit;
}
echo json_encode(['db'=>$_GET['db'],'route'=>$_GET['route'],'session_path'=>session_save_path()]);
""")
    (pma / "js/messages.php").write_text("<?php header('Content-Type: application/javascript'); echo 'const translated = true;';")
    (adminer / "adminer.php").write_text("""<?php
class Adminer {}
$plugin = adminer_object();
if (isset($_POST['auth'])) {
    if (session_status() !== PHP_SESSION_ACTIVE) { http_response_code(500); exit('session closed'); }
    session_regenerate_id(true);
    $_SESSION['pwds']['pgsql']['127.0.0.1'][getenv('PANEL_DB_USER')] = ['encrypted-fixture'];
    $_SESSION['token'] = 'fixture-csrf';
    header('Location: ' . getenv('PANEL_WEB_PREFIX') . '?pgsql=127.0.0.1');
    session_write_close(); exit;
}
if ($_SERVER['REQUEST_METHOD'] === 'POST' && ($_POST['token'] ?? '') !== ($_SESSION['token'] ?? null)) {
    http_response_code(403); exit('Invalid CSRF');
}
echo json_encode(['database'=>$plugin->database(),'databases'=>$plugin->databases(),
    'get'=>$_GET,'credentials'=>$plugin->credentials(),'login'=>$plugin->login(getenv('PANEL_DB_USER'),getenv('PANEL_DB_PASSWORD'))]);
session_write_close();
""")
    cookies = {}
    checks = 0

    def run(engine="postgresql", method="GET", body="", asset="", session="a" * 64, query="", cookie_name="", secure=False):
        env = os.environ.copy()
        env.update({"SCRIPT_FILENAME":str(entry),"REDIRECT_STATUS":"200","REQUEST_METHOD":method,
            "REQUEST_URI":"/api/v1/databases/1/web/", "SCRIPT_NAME":"/api/v1/databases/1/web/index.php",
            "SERVER_PROTOCOL":"HTTP/1.1", "SERVER_NAME":"localhost", "SERVER_PORT":"80", "REMOTE_ADDR":"127.0.0.1",
            "HTTP_HOST":"localhost", "CONTENT_TYPE":"application/x-www-form-urlencoded", "CONTENT_LENGTH":str(len(body)),
            "QUERY_STRING":query,"HTTP_COOKIE":"; ".join(f"{k}={v}" for k,v in cookies.items()),
            "PANEL_DATABASE":"p_alice_shop","PANEL_DB_ENGINE":engine,"PANEL_DB_USER":"qpw_fixture",
            "PANEL_DB_PASSWORD":"Fixture-Safe-Pass!","PANEL_WEB_PREFIX":"/api/v1/databases/1/web/",
            "PANEL_WEB_SESSION":session,"PANEL_WEB_ASSET":asset,
            "PMA_SESSION_COOKIE":cookie_name,"HTTPS":"on" if secure else "off"})
        result = subprocess.run([cgi,"-d","display_errors=0","-d","log_errors=0","-d","session.save_handler=files"], input=body.encode(), env=env, capture_output=True, check=True)
        head, content = result.stdout.split(b"\r\n\r\n",1)
        lines = head.decode().split("\r\n")
        for line in lines:
            if line.lower().startswith("set-cookie:"):
                parsed = http.cookies.SimpleCookie(); parsed.load(line.split(":",1)[1].strip())
                cookies.update({key:value.value for key,value in parsed.items()})
        return lines, content

    def check(value, label):
        global checks
        assert value, label
        checks += 1
        print(f"ok {checks}: {label}")

    headers, _ = run(query="server=evil.test&db=other_database")
    check(any(line.lower().startswith("location:") for line in headers), "first Adminer request bootstraps SSO and rotates its session")
    original = cookies["adminer_sid"]
    headers, content = run(query="server=evil.test&db=other_database")
    data = json.loads(content)
    check(data["get"]["pgsql"] == "127.0.0.1" and data["get"]["db"] == "p_alice_shop", "browser cannot change connection target or database")
    check(data["login"] and data["databases"] == ["p_alice_shop"], "plugin permits only the scoped credentials and database")
    check(cookies["adminer_sid"] == original, "navigation resumes the rotated library session")
    headers, content = run(method="POST", body="token=wrong&sql=SELECT+1")
    check(any("403" in line for line in headers) and b"Invalid CSRF" in content, "subsequent POST retains library CSRF protection")
    headers, content = run(method="POST", body="token=fixture-csrf&sql=SELECT+1")
    check(json.loads(content)["database"] == "p_alice_shop", "valid POST retains the authenticated session")
    headers, content = run(method="POST", body="token=wrong", session="b" * 64)
    check(any("409" in line for line in headers), "expired or different panel session cannot bootstrap SSO during a mutation")
    headers, content = run(engine="mariadb", query="db=other_database")
    data = json.loads(content)
    check(data["db"] == "p_alice_shop" and data["route"] == "/database/structure", "phpMyAdmin opens the requested database structure")
    check(data["session_path"].endswith("a" * 64), "phpMyAdmin shares the panel-scoped storage namespace")
    _, content = run(engine="mariadb", asset="js/messages.php")
    check(content == b"const translated = true;", "phpMyAdmin generated translations execute through the explicit whitelist")
    headers, content = run(engine="mariadb", asset="config.inc.php")
    check(any("404" in line for line in headers), "arbitrary PHP paths are never executed or disclosed")
    headers, content = run(session="invalid")
    check(any("403" in line for line in headers), "malformed gateway context is rejected")
    for name in ["phpMyAdmin_https", "__Secure-phpMyAdmin_https", "phpMyAdmin"]:
        cookies.clear()
        cookies.update({"nusa_session":"panel-secret", "__Secure-nusa_session":"panel-secret",
            "pma_lang_https":"id", "__Secure-pma_lang_https":"id"})
        options = {"engine":"mariadb", "cookie_name":name, "secure":name != "phpMyAdmin"}
        headers, _ = run(**options)
        check(any(line.startswith("Location:") for line in headers) and name in cookies,
            f"{name}: bootstrap creates a rotated PHP session")
        session_id = cookies[name]
        headers, content = run(query="route=/&reload=1", **options)
        check(not any(line.startswith("Location:") for line in headers),
            f"{name}: next request resumes the session instead of redirecting again")
        data = json.loads(content)
        check(cookies[name] == session_id and "nusa_session" not in data["cookies"]
            and "__Secure-nusa_session" not in data["cookies"]
            and "__Secure-pma_lang_https" in data["cookies"],
            f"{name}: only tool cookies reach PHP and session identity stays stable")
        headers, content = run(method="POST", body="token=" + data["token"], **options)
        check(json.loads(content)["token"] == data["token"], f"{name}: valid CSRF survives navigation")
        headers, content = run(method="POST", body="token=wrong", **options)
        check(any("403" in line for line in headers) and b"Invalid CSRF" in content,
            f"{name}: invalid CSRF remains rejected")
    print(f"{checks} PHP bridge fixture checks passed; no database or daemon used.")
