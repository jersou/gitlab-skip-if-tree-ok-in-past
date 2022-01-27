#!/usr/bin/env bash
# From https://gitlab.com/jersou/gitlab-tree-ok-cache/-/blob/skip-version/skip.sh
# Implementation summary :
#     1. Check if the script has already been completed : check ".ci-skip". If file exists: exit 0 if the content == true, otherwise exit 1
#     2. Get the "git ls-tree" of the tree "$SKIP_IF_TREE_OK_IN_PAST" of the current HEAD and generate SHA-1 of this output
#     3. Check if the SHA-1 is present in the ".ci-ok-history"
#     4. If found, write true in ".ci-skip", download and extract the artifact of the found job and exit with code 0
#     6. If not found, append the SHA-1:CI_JOB_ID to ".ci-ok-history" and exit 2
#
# ⚠️ Requirements :
#   - the variable SKIP_IF_TREE_OK_IN_PAST must contain the paths used by the job
#   - docker images/gitlab runner need : bash, curl, git, unzip
#   - if the nested jobs of current uses the dependencies key with current, the dependencies files need to be in an artifact
#   - CI variables changes are not detected. It could be by adding the variables to the tree used to generate the SHA-1.
#
# usage in .gitlab-ci.yml file :
# SERVICE-A:
#   stage: test
#   image: jersou/alpine-bash-curl-git-unzip
#   cache:
#     - key: "${CI_PROJECT_NAMESPACE}__${CI_PROJECT_NAME}__${CI_JOB_NAME}__ci_ok_history"
#       policy: pull-push
#       paths:
#           - ci_ok_history
#   variables:
#       SKIP_IF_TREE_OK_IN_PAST: service-A LIB-1 .gitlab-ci.yml skip.sh
#   script:
#       - ./skip.sh || service-A/test1.sh
#       - ./skip.sh || service-A/test2.sh
#       - ./skip.sh || service-A/test3.sh

set -o errexit

if [[ "$SKIP_IF_TREE_OK_IN_PAST" = "" ]]; then
  echo -e "\e[1;41;39m    ⚠️ The SKIP_IF_TREE_OK_IN_PAST variable is empty, set the list of paths to check    \e[0m"
  exit 1
fi
# 1. Check if the script has already been completed : check "ci-skip". If file exists: exit 0 if the content == true, otherwise exit 1
ci_skip_path="ci-skip-${CI_PROJECT_ID}-${CI_JOB_ID}"
if test -f $ci_skip_path; then
  [[ "$(cat $ci_skip_path)" = "true" ]] && exit 0
  exit 3
fi
# 2. Get the "git ls-tree" of the tree "$SKIP_IF_TREE_OK_IN_PAST" of the current HEAD and generate SHA-1 of this output
current_tree_sha=$(git ls-tree HEAD -- $SKIP_IF_TREE_OK_IN_PAST | tr / \| | git mktree)
echo "false" >$ci_skip_path
echo "skip-if-tree-ok-in-past : current_tree_sha=$current_tree_sha"

# 3. Check if the SHA-1 is present in the history file
job=$(tac ci_ok_history | grep -m 1 "^$current_tree_sha:" | cut -d: -f2)
if [[ "$job" != "" ]] ; then
  # 4. If found, write true in "ci-skip", download and extract the artifact of the found job and exit with code 0
  echo -e "\e[1;43;30m    ✅ tree found in job ${CI_JOB_URL%/*}/$job   \e[0m"
  if [[ "$SKIP_CI_NO_ARTIFACT" != true ]]; then
      curl -o artifact.zip --location "$CI_API_V4_URL/projects/${CI_PROJECT_ID}/jobs/$job/artifacts?job_token=$CI_JOB_TOKEN"
      unzip artifact.zip
      rm artifact.zip
  fi
  echo "true" >$ci_skip_path
  exit 0
else
  # 5. If not found, write false in "ci-skip", append the SHA-1:CI_JOB_ID to "ci_ok_history" and exit 2
  echo "$current_tree_sha:$CI_JOB_ID" >ci_ok_history
  exit 2
fi
