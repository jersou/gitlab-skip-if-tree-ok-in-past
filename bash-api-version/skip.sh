#!/usr/bin/env bash
# From https://gitlab.com/jersou/gitlab-tree-ok-cache/-/blob/skip-version/skip.sh
# Implementation summary :
#  1. Check if the check has already been completed : check /tmp/ci-skip. If file exists, exit, else :
#  2. Get the SHA-1 of the tree "$SKIP_IF_TREE_OK_IN_PAST" of the current HEAD
#  3. Get last 1000 successful jobs of the project
#  4. Filter jobs : keep current job only
#  5. For each job :
#     1. Get the SHA-1 of the tree "$SKIP_IF_TREE_OK_IN_PAST"
#     2. Check if this SHA-1 equals the current HEAD SHA-1 (see 2.)
#     3. If the SHA-1s are equals, write true in /tmp/ci-skip and exit with code 0
#  6. If no job found, write false in /tmp/ci-skip and exit with code > 0
#
# ⚠️ Requirements :
#   - the variable SKIP_IF_TREE_OK_IN_PAST must contain the paths used by the job
#   - docker images/gitlab runner need : bash, curl, git, unzip, fx, NodeJS
#   - if the nested jobs of current uses the dependencies key with current, the dependencies files need to be in an artifact
#   - CI variables changes are not detected
#   - need API_READ_TOKEN (personal access tokens that have read_api scope)
#   - set GIT_DEPTH variable to 1000 or more
#
# usage in .gitlab-ci.yml file :
# SERVICE-A:
#   stage: test
#   image: jersou/alpine-bash-curl-fx-git-nodejs-unzip
#   variables:
#     GIT_DEPTH: 10000
#     SKIP_IF_TREE_OK_IN_PAST: service-A LIB-1 .gitlab-ci.yml skip.sh
#   script:
#     - ./skip.sh || service-A/test1.sh
#     - ./skip.sh || service-A/test2.sh
#     - ./skip.sh || service-A/test3.sh


if [[ "$SKIP_IF_TREE_OK_IN_PAST" = "" ]]; then
  echo -e "\e[1;41;39m    ⚠️ The SKIP_IF_TREE_OK_IN_PAST variable is empty, set the list of paths to check    \e[0m"
  exit 1
fi
if [[ "$API_READ_TOKEN" = "" ]]; then
  echo -e "\e[1;41;39m    ⚠️ The API_READ_TOKEN variable is empty !    \e[0m"
  exit 2
fi
ci_skip_path="/tmp/ci-skip-${CI_PROJECT_ID}-${CI_JOB_ID}"
if test -f $ci_skip_path; then
  [[ "$(cat $ci_skip_path)" = "true" ]] && exit 0
  exit 3
fi

current_tree_sha=$(git ls-tree HEAD -- $SKIP_IF_TREE_OK_IN_PAST | tr / \| | git mktree)

curl --silent --fail "$CI_API_V4_URL/projects/${CI_PROJECT_ID}/jobs?scope=success&per_page=1000&page=&private_token=${API_READ_TOKEN}" |
  fx ".filter(job => job.name === '$CI_JOB_NAME').map(j => [j.commit.id, j.web_url, j.id, j.artifacts_expire_at].join(' ')).join('\n')" |
  while read commit  web_url job artifacts_expire_at ; do
    tree_sha=$(git ls-tree $commit -- $SKIP_IF_TREE_OK_IN_PAST | tr / \| | git mktree)
    if [[ "$tree_sha" = "$current_tree_sha" ]]; then
        echo  "artifacts_expire_at: $artifacts_expire_at"
      if [[ "$artifacts_expire_at" != "" ]]; then
        curl -o artifact.zip --location "$CI_API_V4_URL/projects/${CI_PROJECT_ID}/jobs/$job/artifacts?job_token=$CI_JOB_TOKEN" || break
        unzip artifact.zip
        rm artifact.zip
      fi
      echo -e "\e[1;43;30m    ✅ $current_tree_sha tree found in job $web_url   \e[0m"
      echo true >$ci_skip_path
      break
    fi
  done

if test -f $ci_skip_path; then
  exit 0
else
  echo -e "\e[1;43;30m    ❌ tree not found in last 1000 success jobs of the project    \e[0m"
  echo false >$ci_skip_path
  exit 4
fi
