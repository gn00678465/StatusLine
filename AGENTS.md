## Agent skills

### Issue tracker

Issues and specs are tracked in GitHub Issues. See `docs/agents/issue-tracker.md`.

### Triage labels

Use the five default mattpocock/skills triage labels. See `docs/agents/triage-labels.md`.

### Domain docs

This repository uses a single-context domain-doc layout. See `docs/agents/domain.md`.

### Release gate

For any version release, release tag, GitHub Release, or release-version change,
read `.github/RELEASE_NOTES_TEMPLATE.md` and `.github/workflows/release.yml` in
full before making changes.

1. Start from a clean, up-to-date `main` after every release PR is merged.
2. Before a tag, make one release commit on `main` that keeps all three version
   sources equal to `X.Y.Z`: `claudeStatusLine.sh` `VERSION`, the README version,
   and a `## [vX.Y.Z] - YYYY-MM-DD` CHANGELOG section. Update the compare links.
3. Run the repository test suite, `git diff --check`, and the workflow's
   CHANGELOG-extraction command for `vX.Y.Z`. Commit the verified metadata as
   `chore(release): 發布 vX.Y.Z`, then push `main`.
4. Confirm that neither the remote tag nor GitHub Release already exists. Create
   an annotated `vX.Y.Z` tag at the release commit and push that exact tag.
5. Wait for the tag-triggered `Release` workflow to succeed, then verify the
   published GitHub Release has the expected tag, target commit, and notes.

If a release gate is red, fix it in a new commit before tagging; keep an
existing public tag immutable.
