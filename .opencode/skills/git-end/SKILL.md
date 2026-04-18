---
name: git-end
description: Commit all changes, merge branch into main. Use when finishing work on a feature branch.
---

# Git End - Commit, Merge, and Clean Up

## Workflow

### Step 0: Record Current Branch

```bash
currentBranch=$(git branch --show-current)
```

### Step 1: Review Changes

Review the current changes to generate a meaningful commit message.

```bash
git status
git diff
```

### Step 2: Rebase onto Main

Update main and rebase the current branch on top. Since we're in a worktree, fetch+rebase locally (do NOT `git checkout main`).

```bash
git fetch origin main
git rebase origin/main
```

If there are rebase conflicts:
1. Read each conflicted file and understand the changes from both sides.
2. Resolve conflicts by keeping the correct code from both the feature branch and main.
3. Stage the resolved files: `git add <resolved-files>`
4. Continue the rebase: `git rebase --continue`
5. Repeat until the rebase completes successfully.

### Step 3: Commit All and Push

[[PUSH]] You MUST commit ALL files before pushing. Stage all changes and commit with a semantic message, pull latest, then push to remote.

First, check if there are any uncommitted changes:
```bash
git status
```

If there are uncommitted changes, commit them:
```bash
git add .
git commit -m "<type>: <description>"
```

Then pull latest and push to remote:
```bash
git push origin HEAD -u
git pull
```

The commit message follows the `type: description` convention:
- **feat**: new feature
- **fix**: bug fix
- **refactor**: code restructuring without behavior change
- **chore**: maintenance tasks
- **test**: adding or updating tests
- **docs**: documentation changes

### Step 4: Create Pull Request

Create a pull request immediately after pushing. The CI will run on GitHub via the PR's checks.

```bash
gh pr create --title "<title>" --body "<body>" --base main
```

The title should be the first commit message from your branch:
```bash
TITLE=$(git log main..HEAD --format="%s" | head -1)
```

The body should include all commit messages:
```bash
BODY=$(git log main..HEAD --format="%s" | grep -E '^(feat|fix|refactor|docs|style|test|ci|build|perf|chore)(\(.+\))?!?:' | sed 's/^/- /' | tr '\n' '\n')
```

Example:
```bash
TITLE=$(git log main..HEAD --format="%s" | head -1)
BODY=$(git log main..HEAD --format="%s" | grep -E '^(feat|fix|refactor|docs|style|test|ci|build|perf|chore)(\(.+\))?!?:' | sed 's/^/- /' | tr '\n' '\n')
gh pr create --title "$TITLE" --body "$BODY" --base main
```

### Step 5: Run Local CI

Run local CI via `act` to verify all checks pass. This runs the same CI as GitHub Actions locally to catch issues early.

**IMPORTANT**: The workflow clones the repository via SSH. It will NOT see uncommitted changes in your current worktree. You MUST commit and push all changes before running CI.

```bash
node ./scripts/local_ci.cjs
```

The script will:
- Detect the OS and generate `.actrc` with the correct SSH path
- Run the act workflow with the current branch

Monitor the output. The workflow will:
- Clone repo and checkout the branch
- Run all CI steps (install JS deps, gen types, build JS, cargo test, cargo clippy, typecheck JS, lint JS, wasm-pack build)
- Report success or failure for each step

**If CI fails:**
1. Check the output for the specific error
2. Fix the issues in the current worktree
3. **Re-execute from [[PUSH]]**: Check for uncommitted changes, commit them if any, and push again
4. Re-run act
5. Repeat until CI passes

Do NOT proceed until local CI passes successfully.

### Step 6: Notify User

Inform the user that:
- All changes have been committed and pushed
- Local CI passed
- Pull request has been created at <pr-url>
