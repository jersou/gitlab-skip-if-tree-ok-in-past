#!/usr/bin/env bash

set -o errexit
dir_path=$(dirname "$0")
cd "$dir_path"

rm -rf repo
mkdir -p repo
cd repo

git init .
git config user.name "author"
git config user.email "author@git.git.git"
date="Sat, 12 Mar 2022 15:15:15 +0100"
export GIT_COMMITTER_DATE="$date"
export GIT_AUTHOR_DATE="$date"

mkdir Service-A Service-B
echo 1 > root-1
echo 1 > root-2
echo 1 > Service-A/file-A1
echo 1 > Service-A/file-A2
echo 1 > Service-B/file-B1
echo 1 > Service-B/file-B2
git add .
git commit -m "commit-01"

echo 1 > root-1
echo 2 > root-2
echo 1 > Service-A/file-A1
echo 1 > Service-A/file-A2
echo 2 > Service-B/file-B1
echo 2 > Service-B/file-B2
git add .
git commit -m "commit-02"

echo 1 > root-1
echo 3 > root-2
echo 1 > Service-A/file-A1
echo 1 > Service-A/file-A2
echo 3 > Service-B/file-B1
echo 3 > Service-B/file-B2
git add .
git commit -m "commit-03"

echo 1 > root-1
echo 4 > root-2
echo 1 > Service-A/file-A1
echo 1 > Service-A/file-A2
echo 3 > Service-B/file-B1
echo 3 > Service-B/file-B2
git add .
git commit -m "commit-04"

echo 1 > root-1
echo 5 > root-2
echo 1 > Service-A/file-A1
echo 1 > Service-A/file-A2
echo 3 > Service-B/file-B1
echo 3 > Service-B/file-B2
git add .
git commit -m "commit-05"

echo 1 > root-1
echo 6 > root-2
echo 1 > Service-A/file-A1
echo 1 > Service-A/file-A2
echo 3 > Service-B/file-B1
echo 3 > Service-B/file-B2
git add .
git commit -m "commit-06"

echo 1 > root-1
echo 7 > root-2
echo 1 > Service-A/file-A1
echo 1 > Service-A/file-A2
echo 3 > Service-B/file-B1
echo 3 > Service-B/file-B2
git add .
git commit -m "commit-07"

rm .git/hooks/*

git log
# 26b244f55f8886ccf6a47ea7f24498e9801fc38f commit-07
# 8552ff2454b692432f39aca211eb13e438bbf9c7 commit-06
# c82f05ab9f34bc90f7c2e27413974cbf1f9e7e92 commit-05
# 5e694dadd2979a2680c98c88a2f98df9787947d2 commit-04
# 71caf060ef3022468ffd8b4a70e680d7fec78000 commit-03
# 260d47a1192add224652749a67fa0ac71370b83c commit-02
# ef08d93fdeabf23734248d6f95ab4ff3952e9856 commit-01

git cat-file -p ef08d93fdeabf23734248d6f95ab4ff3952e9856
# tree 5049dd48ff8dad14fb7fcb9fc3139ea8560613c7

git cat-file -p 5049dd48ff8dad14fb7fcb9fc3139ea8560613c7
# 040000 tree 2bd7c857eb491a42e7638cdb7d0f421604359233	Service-A
# 040000 tree bb7c1fc39e500aba9eb3b565c3a56317315491ff	Service-B
# 100644 blob d00491fd7e5bb6fa28c517a0bb32b8b506539d4d	root-1
# 100644 blob d00491fd7e5bb6fa28c517a0bb32b8b506539d4d	root-2

rm ../repo.zip || true
zip -r ../repo.zip .git
tar czvf ../repo.tar.gz .git
cd ..
rm -rf repo
