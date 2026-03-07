#!/bin/bash
set -e

REPO_DIR="$(git rev-parse --show-toplevel)/tmp/test_repo"

rm -rf "$REPO_DIR"
mkdir -p "$REPO_DIR"
cd "$REPO_DIR"

git init -b main
git config user.email "test@test.com"
git config user.name "Test User"

# ============================================================
# MAIN: initial commits
# ============================================================

cat > README.md << 'EOF'
# Test Project
A sample project for testing merge commit handling.
EOF
git add README.md
git commit -m "Initial commit: add README"

cat > auth.py << 'EOF'
def login(username, password):
    if username == "admin" and password == "secret":
        return True
    return False

def logout(session):
    session.clear()
EOF
git add auth.py
git commit -m "Add authentication module"

cat > database.py << 'EOF'
class Database:
    def __init__(self, connection_string):
        self.conn = connection_string
        self.connected = False

    def connect(self):
        self.connected = True

    def query(self, sql):
        if not self.connected:
            raise Exception("Not connected")
        return []
EOF
git add database.py
git commit -m "Add database module"

cat > utils.py << 'EOF'
def format_date(date):
    return date.strftime("%Y-%m-%d")

def parse_csv(text):
    return [line.split(",") for line in text.strip().split("\n")]

def sanitize_input(text):
    return text.strip().replace("<", "&lt;").replace(">", "&gt;")
EOF
git add utils.py
git commit -m "Add utility functions"

# ============================================================
# FEATURE BRANCH: diverge from main
# ============================================================

git checkout -b feature

# Feature commit 1: add API layer
cat > api.py << 'EOF'
from auth import login
from database import Database

class API:
    def __init__(self):
        self.db = Database("sqlite:///app.db")

    def authenticate(self, username, password):
        return login(username, password)

    def get_users(self):
        self.db.connect()
        return self.db.query("SELECT * FROM users")
EOF
git add api.py
git commit -m "Add API layer"

# Feature commit 2: add tests
cat > tests.py << 'EOF'
import unittest
from auth import login

class TestAuth(unittest.TestCase):
    def test_valid_login(self):
        self.assertTrue(login("admin", "secret"))

    def test_invalid_login(self):
        self.assertFalse(login("user", "wrong"))

if __name__ == "__main__":
    unittest.main()
EOF
git add tests.py
git commit -m "Add unit tests for auth"

# ============================================================
# MAIN: advance with non-conflicting changes
# ============================================================

git checkout main

# Main commit: add config file (no conflict)
cat > config.py << 'EOF'
DEBUG = False
DATABASE_URL = "sqlite:///production.db"
SECRET_KEY = "change-me-in-production"
MAX_RETRIES = 3
EOF
git add config.py
git commit -m "Add config module"

# Main commit: edit utils.py (no conflict with feature — feature didn't touch utils)
cat > utils.py << 'EOF'
import re

def format_date(date):
    return date.strftime("%Y-%m-%d")

def format_datetime(date):
    return date.strftime("%Y-%m-%d %H:%M:%S")

def parse_csv(text):
    return [line.split(",") for line in text.strip().split("\n")]

def sanitize_input(text):
    return text.strip().replace("<", "&lt;").replace(">", "&gt;")

def validate_email(email):
    return bool(re.match(r"^[\w.-]+@[\w.-]+\.\w+$", email))
EOF
git add utils.py
git commit -m "Extend utils with format_datetime and validate_email"

# ============================================================
# MERGE 1: Clean merge of main into feature (no conflicts)
# ============================================================

git checkout feature
git merge main --no-ff -m "Merge main into feature (clean)"

# Feature commit 3: more work after clean merge
cat > middleware.py << 'EOF'
class RateLimiter:
    def __init__(self, max_requests=100, window=60):
        self.max_requests = max_requests
        self.window = window
        self.requests = {}

    def is_allowed(self, client_ip):
        # Simplified rate limiting
        count = self.requests.get(client_ip, 0)
        if count >= self.max_requests:
            return False
        self.requests[client_ip] = count + 1
        return True
EOF
git add middleware.py
git commit -m "Add rate limiter middleware"

# ============================================================
# MAIN: make changes that WILL conflict with feature
# ============================================================

git checkout main

# Main edits auth.py (feature also uses auth.py via api.py import, but
# the CONFLICT comes from both editing the same file lines)
cat > auth.py << 'EOF'
import hashlib

def login(username, password):
    hashed = hashlib.sha256(password.encode()).hexdigest()
    if username == "admin" and hashed == "2bb80d537b1da3e38bd30361aa855686bde0eacd7162fef6a25fe97bf527a25b":
        return True
    return False

def logout(session):
    session.clear()
    session.invalidate()

def reset_password(username, new_password):
    hashed = hashlib.sha256(new_password.encode()).hexdigest()
    return {"username": username, "password_hash": hashed}
EOF
git add auth.py
git commit -m "Hash passwords in auth module"

# Main also edits database.py in a way that will conflict
cat > database.py << 'EOF'
import logging

logger = logging.getLogger(__name__)

class Database:
    def __init__(self, connection_string):
        self.conn = connection_string
        self.connected = False
        self.pool_size = 5

    def connect(self):
        logger.info("Connecting to database")
        self.connected = True

    def disconnect(self):
        logger.info("Disconnecting from database")
        self.connected = False

    def query(self, sql):
        if not self.connected:
            raise Exception("Not connected")
        logger.debug(f"Executing: {sql}")
        return []

    def execute(self, sql, params=None):
        if not self.connected:
            raise Exception("Not connected")
        logger.debug(f"Executing: {sql} with {params}")
        return True
EOF
git add database.py
git commit -m "Add logging and connection pooling to database"

# ============================================================
# FEATURE: edit the same files to create conflicts
# ============================================================

git checkout feature

# Feature edits auth.py differently (CONFLICT with main's hashed version)
cat > auth.py << 'EOF'
from database import Database

def login(username, password):
    db = Database("sqlite:///app.db")
    db.connect()
    users = db.query(f"SELECT * FROM users WHERE username='{username}'")
    if users and users[0]["password"] == password:
        return True
    return False

def logout(session):
    session.clear()
    print("User logged out")

def create_user(username, password):
    db = Database("sqlite:///app.db")
    db.connect()
    return db.query(f"INSERT INTO users VALUES ('{username}', '{password}')")
EOF
git add auth.py
git commit -m "Refactor auth to use database lookups"

# Feature also edits database.py differently (CONFLICT)
cat > database.py << 'EOF'
class Database:
    def __init__(self, connection_string):
        self.conn = connection_string
        self.connected = False
        self.transaction_active = False

    def connect(self):
        self.connected = True

    def begin_transaction(self):
        self.transaction_active = True

    def commit_transaction(self):
        self.transaction_active = False

    def rollback(self):
        self.transaction_active = False

    def query(self, sql):
        if not self.connected:
            raise Exception("Not connected")
        return []
EOF
git add database.py
git commit -m "Add transaction support to database"

# ============================================================
# MERGE 2: Conflicting merge of main into feature
# ============================================================

# This merge will have conflicts in auth.py and database.py
git merge main --no-ff -m "Merge main into feature (with conflicts)" || true

# Resolve conflicts manually
cat > auth.py << 'EOF'
import hashlib
from database import Database

def login(username, password):
    """Database-backed login with hashed passwords."""
    hashed = hashlib.sha256(password.encode()).hexdigest()
    db = Database("sqlite:///app.db")
    db.connect()
    users = db.query(f"SELECT * FROM users WHERE username='{username}'")
    if users and users[0]["password_hash"] == hashed:
        return True
    return False

def logout(session):
    session.clear()
    session.invalidate()

def create_user(username, password):
    hashed = hashlib.sha256(password.encode()).hexdigest()
    db = Database("sqlite:///app.db")
    db.connect()
    return db.query(f"INSERT INTO users (username, password_hash) VALUES ('{username}', '{hashed}')")

def reset_password(username, new_password):
    hashed = hashlib.sha256(new_password.encode()).hexdigest()
    return {"username": username, "password_hash": hashed}
EOF

cat > database.py << 'EOF'
import logging

logger = logging.getLogger(__name__)

class Database:
    def __init__(self, connection_string):
        self.conn = connection_string
        self.connected = False
        self.pool_size = 5
        self.transaction_active = False

    def connect(self):
        logger.info("Connecting to database")
        self.connected = True

    def disconnect(self):
        logger.info("Disconnecting from database")
        self.connected = False

    def begin_transaction(self):
        self.transaction_active = True

    def commit_transaction(self):
        self.transaction_active = False

    def rollback(self):
        self.transaction_active = False

    def query(self, sql):
        if not self.connected:
            raise Exception("Not connected")
        logger.debug(f"Executing: {sql}")
        return []

    def execute(self, sql, params=None):
        if not self.connected:
            raise Exception("Not connected")
        logger.debug(f"Executing: {sql} with {params}")
        return True
EOF

git add auth.py database.py
git commit -m "Merge main into feature (with conflicts)"

# ============================================================
# Feature commit after conflict merge: more work
# ============================================================

cat > api.py << 'EOF'
from auth import login, create_user
from database import Database
from middleware import RateLimiter

class API:
    def __init__(self):
        self.db = Database("sqlite:///app.db")
        self.limiter = RateLimiter()

    def authenticate(self, username, password):
        return login(username, password)

    def register(self, username, password):
        return create_user(username, password)

    def get_users(self):
        self.db.connect()
        return self.db.query("SELECT * FROM users")

    def health_check(self):
        return {"status": "ok"}
EOF
git add api.py
git commit -m "Update API to use new auth and add registration"

# ============================================================
# MAIN: more changes for another merge
# ============================================================

git checkout main

cat > logger.py << 'EOF'
import logging
import sys

def setup_logging(level="INFO"):
    handler = logging.StreamHandler(sys.stdout)
    handler.setLevel(getattr(logging, level))
    formatter = logging.Formatter("%(asctime)s - %(name)s - %(levelname)s - %(message)s")
    handler.setFormatter(formatter)
    logging.root.addHandler(handler)
    logging.root.setLevel(getattr(logging, level))
EOF
git add logger.py
git commit -m "Add centralized logging setup"

# Edit config.py on main (no conflict — feature hasn't touched it since merge)
cat > config.py << 'EOF'
import os

DEBUG = os.environ.get("DEBUG", "false").lower() == "true"
DATABASE_URL = os.environ.get("DATABASE_URL", "sqlite:///production.db")
SECRET_KEY = os.environ.get("SECRET_KEY", "change-me-in-production")
MAX_RETRIES = int(os.environ.get("MAX_RETRIES", "3"))
LOG_LEVEL = os.environ.get("LOG_LEVEL", "INFO")
EOF
git add config.py
git commit -m "Read config from environment variables"

# ============================================================
# MERGE 3: Another clean merge of main into feature
# ============================================================

git checkout feature
git merge main --no-ff -m "Merge main into feature (clean, pick up logging)"

# Feature: final commits
cat > tests.py << 'EOF'
import unittest
from auth import login, create_user
from database import Database
from api import API

class TestAuth(unittest.TestCase):
    def test_valid_login(self):
        # Note: this would need a real DB setup to pass
        result = login("admin", "secret")
        self.assertIsNotNone(result)

    def test_invalid_login(self):
        result = login("user", "wrong")
        self.assertFalse(result)

class TestDatabase(unittest.TestCase):
    def test_connect(self):
        db = Database("sqlite:///test.db")
        db.connect()
        self.assertTrue(db.connected)

    def test_transaction(self):
        db = Database("sqlite:///test.db")
        db.connect()
        db.begin_transaction()
        self.assertTrue(db.transaction_active)
        db.rollback()
        self.assertFalse(db.transaction_active)

class TestAPI(unittest.TestCase):
    def test_health_check(self):
        api = API()
        result = api.health_check()
        self.assertEqual(result["status"], "ok")

if __name__ == "__main__":
    unittest.main()
EOF
git add tests.py
git commit -m "Expand test suite with database and API tests"

cat > cache.py << 'EOF'
from collections import OrderedDict

class LRUCache:
    def __init__(self, capacity=128):
        self.capacity = capacity
        self.cache = OrderedDict()

    def get(self, key):
        if key in self.cache:
            self.cache.move_to_end(key)
            return self.cache[key]
        return None

    def put(self, key, value):
        if key in self.cache:
            self.cache.move_to_end(key)
        self.cache[key] = value
        if len(self.cache) > self.capacity:
            self.cache.popitem(last=False)

    def clear(self):
        self.cache.clear()
EOF
git add cache.py
git commit -m "Add LRU cache implementation"

# ============================================================
# Summary
# ============================================================

echo ""
echo "Test repo created at: $REPO_DIR"
echo ""
echo "=== Feature branch log (feature) ==="
git log --oneline --graph feature
echo ""
echo "=== Main branch log ==="
git log --oneline --graph main
echo ""
echo "Branch structure:"
echo "  - main: 8 commits"
echo "  - feature: 8 feature commits + 3 merges from main"
echo "    - Merge 1: clean (no conflicts)"
echo "    - Merge 2: CONFLICTED (auth.py + database.py)"
echo "    - Merge 3: clean (no conflicts)"
