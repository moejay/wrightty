import subprocess, time, os, signal, pytest
from wrightty.terminal import Terminal
from wrightty.client import WrighttyClient

WRIGHTTY_BIN = os.environ.get("WRIGHTTY_BIN", "wrightty")

@pytest.fixture
def server():
    """Start a headless wrightty server for testing."""
    proc = subprocess.Popen(
        [WRIGHTTY_BIN, "term", "--headless", "--port", "19420"],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    time.sleep(1)
    yield "ws://127.0.0.1:19420"
    proc.send_signal(signal.SIGTERM)
    proc.wait(timeout=5)

@pytest.fixture
def auth_server():
    """Start a headless wrightty server with password."""
    proc = subprocess.Popen(
        [WRIGHTTY_BIN, "term", "--headless", "--port", "19421", "--password", "secret123"],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    time.sleep(1)
    yield "ws://127.0.0.1:19421"
    proc.send_signal(signal.SIGTERM)
    proc.wait(timeout=5)

def test_connect_and_info(server):
    term = Terminal.connect(url=server)
    info = term.get_info()
    assert "version" in info or "implementation" in info
    term.close()

def test_discover_finds_server(server):
    servers = Terminal.discover()
    urls = [s["url"] for s in servers]
    assert server in urls

def test_auth_required_without_password(auth_server):
    with pytest.raises(ConnectionError, match="password"):
        Terminal.connect(url=auth_server)

def test_auth_with_correct_password(auth_server):
    term = Terminal.connect(url=auth_server, password="secret123")
    info = term.get_info()
    assert info is not None
    term.close()

def test_auth_with_wrong_password(auth_server):
    with pytest.raises(Exception):
        Terminal.connect(url=auth_server, password="wrongpassword")
