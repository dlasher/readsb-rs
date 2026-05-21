# GitHub Automated Releases Implementation Plan Template

> For agentic workers: REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax for tracking.

Goal: Build and publish {{PROJECT_NAME}} binaries for {{ARCHITECTURE_LIST}} on every {{TAG_PATTERN}} tag push.

Architecture: Single GitHub Actions workflow with a build matrix for {{ARCHITECTURE_COUNT}} architectures. {{COMPILER_DETAILS}} cross-compilation. Releases via softprops/action-gh-release.

Tech Stack: GitHub Actions, {{LANGUAGE_AND_VERSION}}, softprops/action-gh-release

----------------------------------------------------------------------

### Task 1: Create release workflow

Files:
- Create: .github/workflows/{{WORKFLOW_FILENAME}}.yml

- [ ] Step 1: Create the workflow file

Write .github/workflows/{{WORKFLOW_FILENAME}}.yml:

--- YAML CONTENT START ---
name: {{WORKFLOW_NAME}}

on:
  push:
    tags:
      - '{{TAG_PATTERN}}'

jobs:
  build:
    name: Build ${{ matrix.arch }}
    runs-on: ubuntu-latest
    strategy:
      matrix:
        goos: [{{OS_LIST}}]
        goarch: [{{GOARCH_LIST}}]
        include:
          - goarch: arm
            goarm: "6"

    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-go@v5
        with:
          go-version: '{{LANGUAGE_VERSION}}'
          check-latest: true

      - name: Build
        run: |
          mkdir -p dist
          GOOS=${{ matrix.goos }} GOARCH=${{ matrix.goarch }} GOARM=${{ matrix.goarm }} \
          CGO_ENABLED={{CGO_ENABLED_FLAG}} \
          go build -ldflags "-X main.version=${GITHUB_REF_NAME#v} -X main.commit=${{ github.sha }} -X main.date=$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
          -o dist/{{PROJECT_NAME}}_${GITHUB_REF_NAME}_${{ matrix.goos }}_${{ matrix.goarch }} {{MAIN_PACKAGE_PATH}}

      - name: Archive
        run: |
          cd dist
          tar czf {{PROJECT_NAME}}_${GITHUB_REF_NAME}_${{ matrix.goos }}_${{ matrix.goarch }}.tar.gz \
            {{PROJECT_NAME}}_${GITHUB_REF_NAME}_${{ matrix.goos }}_${{ matrix.goarch }}

      - uses: softprops/action-gh-release@v2
        if: startsWith(github.ref, 'refs/tags/')
        with:
          files: dist/*.tar.gz
          generate_release_notes: true
--- YAML CONTENT END ---


- [ ] Step 2: Create the directory and save the file

Command:
mkdir -p .github/workflows

Write .github/workflows/{{WORKFLOW_FILENAME}}.yml with the content above.

- [ ] Step 3: Verify the file exists

Command:
ls -la .github/workflows/{{WORKFLOW_FILENAME}}.yml

Expected output: .github/workflows/{{WORKFLOW_FILENAME}}.yml exists and is readable.

- [ ] Step 4: Stage and commit

Command:
git add .github/workflows/{{WORKFLOW_FILENAME}}.yml docs/superpowers/specs/{{DATE}}-github-releases-design.md docs/superpowers/plans/{{DATE}}-github-releases-plan.md
git commit -m "ci: add automated release workflow for {{ARCHITECTURE_LIST}}"

- [ ] Step 5: Push to origin

Command:
git push origin main

- [ ] Step 6: Verify the workflow is visible on GitHub

Action:
Open https://github.com/{{GITHUB_USER_OR_ORG}}/{{GITHUB_REPO}}/actions in a browser. The "{{WORKFLOW_NAME}}" workflow should appear in the left sidebar.

- [ ] Step 7: Create a test tag to trigger a release

Commands:
git tag {{TEST_TAG_NAME}}
git push origin {{TEST_TAG_NAME}}

Expected: The Release workflow triggers on GitHub Actions. Check:
- https://github.com/{{GITHUB_USER_OR_ORG}}/{{GITHUB_REPO}}/actions -- build running
- https://github.com/{{GITHUB_USER_OR_ORG}}/{{GITHUB_REPO}}/releases -- release appears after completion
