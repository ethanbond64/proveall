#!/bin/bash
set -e

REPO_DIR="$(git rev-parse --show-toplevel)/tmp/test_repo"

rm -rf "$REPO_DIR"
mkdir -p "$REPO_DIR"
cd "$REPO_DIR"

git init
git config user.email "test@test.com"
git config user.name "Test User"

# Main: first two commits
echo "file A content" > file_a.txt
git add file_a.txt
git commit -m "main: add file_a"

echo "file B content" > file_b.txt
git add file_b.txt
git commit -m "main: add file_b"

# Create feature branch
git checkout -b feature

# Feature: add file C
echo "file C content" > file_c.txt
git add file_c.txt
git commit -m "feature: add file_c"

# Back to main: edit file_a
git checkout main
echo "file A content (edited on main)" > file_a.txt
git add file_a.txt
git commit -m "main: edit file_a"

# Back to feature: add file D
git checkout feature
echo "file D content" > file_d.txt
git add file_d.txt
git commit -m "feature: add file_d"

# Merge main into feature
git merge main -m "feature: merge main into feature"

# Add another commit to main (edit file_b, no conflict with feature)
git checkout main
echo "file B content (edited on main after merge)" > file_b.txt
git add file_b.txt
git commit -m "main: edit file_b after feature diverged"

echo ""
echo "Test repo created at: $REPO_DIR"
echo ""
echo "=== main log ==="
git log --oneline --graph main
echo ""
echo "=== feature log ==="
git log --oneline --graph feature
