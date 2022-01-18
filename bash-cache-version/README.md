# Add "skip if sub tree is ok in the past" job option, useful in monorepos ~= Idempotent job

* GitLab issue : https://gitlab.com/gitlab-org/gitlab/-/issues/350212
* working API version : https://gitlab.com/jersou/gitlab-skip-if-tree-ok-in-past


### Problem to solve

On monorepo projects (especially), the jobs are run all the time, even
if their state has already been successfully run previously. Time and
resources could be saved by checking that the version of the files used
by the job has already succeeded in the past.

### Proposal

An option in `.gtlab-ci.yml` file "idempotent_tree" (name to be determined)
with an array of paths could be used to make a history of state that have
passed the job with success:

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
it uses gitlab cache and `git ls-tree` & `git mktree` to generate the SHA-1 of the "state" :

```yaml
  # allow the 222 exit code : allow failure if tree is found in history
  allow_failure:
    exit_codes:
      - 222
  variables:
    TREE_TO_CHECK: service-A/ LIB-1/ LIB-2/ .gitlab-ci.yml
  before_script:
    # skip the job if the SHA-1 of the "$TREE_TO_CHECK" tree is in the history file
    - |
      ! grep "^$(git ls-tree HEAD -- $TREE_TO_CHECK | tr / \| | git mktree):" .ci_ok_history \
      || exit 222
  after_script:
    # if job is successful, add the SHA-1 of the "$TREE_TO_CHECK" tree to the history file
    - |
      [ "$CI_JOB_STATUS" = success ] \
       && echo $(git ls-tree HEAD -- $TREE_TO_CHECK | tr / \| | git mktree):${CI_JOB_ID} >> .ci_ok_history
```

The command `git ls-tree HEAD -- $TREE_TO_CHECK` outputs :

```bash
100644 blob da36badb1ae56b374363b413a332b288e76415ab	.gitlab-ci.yml
100755 blob 88e89803687ebf9ec2942c286786530bcf8c4c8c	LIB-1/test.sh
100755 blob fa60bad0352c64ac2e20ee210be0d96556f38cec	LIB-2/test.sh
100755 blob 4586c34e690276e3a848ae72ad231325dd184355	service-A/test.sh
```

Then, the command `git ls-tree HEAD -- $TREE_TO_CHECK | tr / \| | git mktree`
outputs the SHA-1 of `$TREE_TO_CHECK` : `70552b00d642bfa259b1622674e85844d8711ad6`

This SHA-1 is searched in the `.ci_ok_history` file, if it is found, the script stops
with the code 222 (allowed), otherwise the job script continues.

If the job is successful, the SHA-1 is added to the `.ci_ok_history` file. This file is cached:

```
  cache:
    key: "${CI_PROJECT_NAMESPACE}__${CI_PROJECT_NAME}__${CI_JOB_NAME}__ci_ok_history"
    policy: pull-push
    untracked: true
    paths:
      - .ci_ok_history
```

This POC work fine, but need git in the docker image, and it would be much more
graceful if it was integrated in gitlab of course.

### Further details

If this idea is implemented in gitlab, the problem of artifacts should be addressed,
perhaps a link could be made to the artifact of the job that was found in the history.
And if the artifacts are outdated, then the current job is finally executed to produce
a new artifact (possibly activated/deactivated by an option).

Or the job could be skipped like the "only:changes" option.

### Links / references

[jersou / Gitlab Tree Ok Cache · GitLab](https://gitlab.com/jersou/gitlab-tree-ok-cache)

/label ~"feature::addition" ~"type::feature" ~"CI jobs"
