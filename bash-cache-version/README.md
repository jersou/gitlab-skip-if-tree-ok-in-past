
See ../.gitlab-ci.yml file exemple, job ".bash-api-version-ci-history", "SERVICE-A-cache" and "SERVICE-B-cache"




This implementation uses gitlab cache and `git ls-tree` & `git mktree` to generate the SHA-1 of the "state" :

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

