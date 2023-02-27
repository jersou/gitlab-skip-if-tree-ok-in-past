# gitlab-skip-if-tree-ok-in-past tools

This project contains 3 implementation of the issue described bellow :

* a bash implementation, that require : bash, curl, git, unzip, fx
* a NodeJS implementation, that require : git, nodejs, unzip
* a Go implementation, that require : nothing except the 1.9Mo binary file "
  skip-if-tree-ok-in-past"

The POC describe in the issue bellow use gitlab-ci job cache to find OK trees,
but the implementations of the current projet use Gitlab API:

### Implementation summary :

1. Check if the script has already been completed : check ci-skip file. If file
   exists, exit, else :
2. Get the "git ls-tree" of the tree "SKIP_IF_TREE_OK_IN_PAST" of the current
   HEAD
3. Get last successful jobs of the project
4. Filter jobs : keep current job only
5. For each job :
    1. Get the "git ls-tree" of the tree "SKIP_IF_TREE_OK_IN_PAST"
    2. Check if this "git ls-tree" equals the current HEAD "git ls-tree" ( ⇧ 2.)
    3. If the "git ls-tree" are equals, write true in ci-skip file and exit with
       code 0
6. If no job found, write false in ci-skip file and exit with code > 0

### ⚠️ Warning/Requirements :

- the variable `SKIP_IF_TREE_OK_IN_PAST` must contain the paths used by the job
- need `API_READ_TOKEN` (personal access tokens that have `read_api` scope)
- set `GIT_DEPTH` variable to 1000 or more
- if the nested jobs of current uses the dependencies key with current, the
  dependencies files need to be in an artifact
- CI variables changes are not detected (Trees will be considered equal despite
  changes in variables)

### Usage in .gitlab-ci.yml file :

```
SERVICE-A:
   stage: test
   image: alpine
   variables:
      GIT_DEPTH: 1000
      SKIP_IF_TREE_OK_IN_PAST: service-A LIB-1 .gitlab-ci.yml skip.sh
   script:
      - ./skip-if-tree-ok-in-past || service-A/test1.sh
      - ./skip-if-tree-ok-in-past || service-A/test2.sh
      - ./skip-if-tree-ok-in-past || service-A/test3.sh
```

## [The gitlab issue #350212 :](https://gitlab.com/gitlab-org/gitlab/-/issues/350212) Add "skip if sub tree is ok in the past" job option, useful in monorepos ~= Idempotent job

### Problem to solve

On monorepo projects (especially), the jobs are run all the time, even if their
state has already been successfully run previously. Time and resources could be
saved by checking that the version of the files used by the job has already
succeeded in the past.

### Proposal

An option in `.gtlab-ci.yml` file "idempotent_tree" (name to be determined)
with an array of paths could be used to make a history of state that have passed
the job with success:

```
service-A:
  idempotent_tree:
    - service-A/
    - LIB-1/
    - LIB-2/
    - .gitlab-ci.yml
  script:
    - service-A/test.sh
```

A POC of this idea is operational here
[jersou / Gitlab Tree Ok Cache](https://gitlab.com/jersou/gitlab-tree-ok-cache),
it uses gitlab cache and `git ls-tree` & `git mktree` to generate the SHA-1 of
the "state" :

```yaml
  # allow the 222 exit code : allow failure if tree is found in history
  allow_failure:
    exit_codes:
      - 222
  variables:
    SKIP_IF_TREE_OK_IN_PAST: service-A/ LIB-1/ LIB-2/ .gitlab-ci.yml
  before_script:
    # skip the job if the SHA-1 of the "$SKIP_IF_TREE_OK_IN_PAST" tree is in the history file
    - |
      ! grep "^$(git ls-tree HEAD -- $SKIP_IF_TREE_OK_IN_PAST | tr / \| | git mktree):" ci_ok_history \
      || exit 222
  after_script:
    # if job is successful, add the SHA-1 of the "$SKIP_IF_TREE_OK_IN_PAST" tree to the history file
    - |
      [ "$CI_JOB_STATUS" = success ] \
       && echo $(git ls-tree HEAD -- $SKIP_IF_TREE_OK_IN_PAST | tr / \| | git mktree):${CI_JOB_ID} >> ci_ok_history
```

The command `git ls-tree HEAD -- $SKIP_IF_TREE_OK_IN_PAST` outputs :

```bash
100644 blob da36badb1ae56b374363b413a332b288e76415ab	.gitlab-ci.yml
100755 blob 88e89803687ebf9ec2942c286786530bcf8c4c8c	LIB-1/test.sh
100755 blob fa60bad0352c64ac2e20ee210be0d96556f38cec	LIB-2/test.sh
100755 blob 4586c34e690276e3a848ae72ad231325dd184355	service-A/test.sh
```

Then, the
command `git ls-tree HEAD -- $SKIP_IF_TREE_OK_IN_PAST | tr / \| | git mktree`
outputs the SHA-1
of `$SKIP_IF_TREE_OK_IN_PAST` : `70552b00d642bfa259b1622674e85844d8711ad6`

This SHA-1 is searched in the `ci_ok_history` file, if it is found, the script
stops with the code 222 (allowed), otherwise the job script continues.

If the job is successful, the SHA-1 is added to the `ci_ok_history` file. This
file is cached:

```
  cache:
    key: "${CI_PROJECT_NAMESPACE}__${CI_PROJECT_NAME}__${CI_JOB_NAME}__ci_ok_history"
    policy: pull-push
    paths:
      - ci_ok_history
```

This POC work fine, but need git in the docker image, and it would be much more
graceful if it was integrated in gitlab of course.

### Further details

If this idea is implemented in gitlab, the problem of artifacts should be
addressed, perhaps a link could be made to the artifact of the job that was
found in the history. And if the artifacts are outdated, then the current job is
finally executed to produce a new artifact (possibly activated/deactivated by an
option).

Or the job could be skipped like the "only:changes" option.

#### Skip version implementation (see skip-version branch)

1. Check if the process has already been completed : check file ci-skip file. If
   file found, exit, else :
2. Get the SHA-1 of the tree "$SKIP_IF_TREE_OK_IN_PAST" of the current HEAD
3. Get last 1000 successful jobs of the project
4. Filter jobs : keep current job only
5. For each job :
    1. Get the SHA-1 of the tree "$SKIP_IF_TREE_OK_IN_PAST"
    2. Check if this SHA-1 equals the current HEAD SHA-1 (see 2.)
    3. If the SHA-1s are equals, write true in ci-skip file and exit with code 0
6. If no job found, write false in ci-skip file and exit with code > 0

### Links / references

- https://gitlab.com/jersou/gitlab-tree-ok-cache
- https://gitlab.com/jersou/gitlab-skip-if-tree-ok-in-past
- https://github.com/jersou/gitlab-skip-if-tree-ok-in-past


## Requirements/versions matrix

Requirement by versions :

|        | Cache                                                     | API                                                          |
|--------|-----------------------------------------------------------|--------------------------------------------------------------|
| Bash   | bash, curl, git, unzip                                    | bash, curl, git, unzip, fx                                   |
| Node   | nodejs, unzip, git                                        | nodejs, unzip, git                                           |
| Deno   | N/A                                                       | deno, unzip, git                                             |
| Go     | **none !**                                                | **none !**                                                   |
| Global | SKIP_IF_TREE_OK_IN_PAST variable,<br>ci-skip gitlab cache | SKIP_IF_TREE_OK_IN_PAST variable,<br>API_READ_TOKEN variable |

→ the go version "embeds" all requirements (git/unzip/http)
