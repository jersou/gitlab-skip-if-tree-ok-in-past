#!/usr/bin/env bash

set -o errexit
dir_path=$(dirname "$0")
cd "$dir_path/.."

tmp_dir=$(mktemp --directory)
cp skip-if-tree-ok-in-past $tmp_dir
tar -zxf test/repo.tar.gz --directory "$tmp_dir"
cd "$tmp_dir"

export CI_API_V4_URL=http://localhost/api
export SKIP_IF_TREE_OK_IN_PAST=root-1
export API_READ_TOKEN=___API_READ_TOKEN___
export CI_PROJECT_ID=123
export CI_JOB_ID=456
export CI_COMMIT_REF_NAME=branch2
export CI_PROJECT_DIR=$tmp_dir
export CI_JOB_NAME=jobA
export SKIP_CI_PAGE_TO_FETCH_MAX=1
export CI_JOB_TOKEN=___CI_JOB_TOKEN___

./skip-if-tree-ok-in-past
ls -al
echo "ci-skip=$(cat ci-skip-123-456)"

rm -rf "$tmp_dir"
