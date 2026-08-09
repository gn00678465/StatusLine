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

1. Prepare one release PR from a clean, up-to-date `main`. A feature PR may be
   that release PR when the user has named its target version.
2. Before that PR is merged, keep all three version sources equal to `X.Y.Z`:
   `claudeStatusLine.sh` `VERSION`, the README version, and a
   `## [vX.Y.Z] - YYYY-MM-DD` CHANGELOG section. Leave `Unreleased` empty and
   update the compare links.
3. In the release PR, run the repository test suite, `git diff --check`, and the
   workflow's CHANGELOG-extraction command for `vX.Y.Z`. Print the verified
   version values, extracted notes, and relevant mock output for user approval.
4. Merge the approved release PR, update local `main`, and verify its merge
   commit contains those exact version values and extracted notes.
5. Confirm that neither the remote tag nor GitHub Release already exists. Create
   an annotated `vX.Y.Z` tag at that verified `main` commit and push that exact
   tag.
6. Wait for the tag-triggered `Release` workflow to succeed, then verify the
   published GitHub Release has the expected tag, target commit, and notes.

If a release gate is red, fix it in a new commit before tagging; keep an
existing public tag immutable.
