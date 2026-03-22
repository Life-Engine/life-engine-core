#!/usr/bin/env bash
set -euo pipefail

# configure-branch-protection.sh
#
# Applies GitHub branch protection rules to the main branch of the
# Life Engine repository. Requires the GitHub CLI (gh) to be installed
# and authenticated with sufficient permissions (repo admin).

BRANCH="main"

# ---------------------------------------------------------------------------
# Preflight checks
# ---------------------------------------------------------------------------

if ! command -v gh &>/dev/null; then
  echo "ERROR: GitHub CLI (gh) is not installed."
  echo "       Install it from https://cli.github.com/ and try again."
  exit 1
fi

if ! gh auth status &>/dev/null; then
  echo "ERROR: GitHub CLI is not authenticated."
  echo "       Run 'gh auth login' first."
  exit 1
fi

# ---------------------------------------------------------------------------
# Auto-detect repository owner/name
# ---------------------------------------------------------------------------

REPO=$(gh repo view --json nameWithOwner --jq '.nameWithOwner' 2>/dev/null) || true

if [[ -z "${REPO:-}" ]]; then
  echo "ERROR: Could not detect repository owner/name."
  echo "       Make sure you are inside a Git repository with a GitHub remote."
  exit 1
fi

echo "Repository: ${REPO}"
echo "Branch:     ${BRANCH}"
echo ""

# ---------------------------------------------------------------------------
# Apply branch protection rules
# ---------------------------------------------------------------------------

echo "Applying branch protection rules..."

gh api "repos/${REPO}/branches/${BRANCH}/protection" \
  --method PUT \
  --input - <<'PAYLOAD'
{
  "required_status_checks": {
    "strict": true,
    "contexts": [
      "Rust Checks",
      "DCO Check"
    ]
  },
  "required_pull_request_reviews": {
    "dismiss_stale_reviews": true,
    "required_approving_review_count": 1
  },
  "restrictions": null,
  "required_linear_history": true,
  "allow_force_pushes": false,
  "allow_deletions": false,
  "required_conversation_resolution": true,
  "enforce_admins": true
}
PAYLOAD

echo ""
echo "Branch protection rules applied successfully."
echo ""

# ---------------------------------------------------------------------------
# Verification — read the rules back
# ---------------------------------------------------------------------------

echo "Verifying applied rules..."
echo ""

PROTECTION=$(gh api "repos/${REPO}/branches/${BRANCH}/protection" 2>/dev/null) || {
  echo "WARNING: Could not read back branch protection rules for verification."
  echo "         The PUT request succeeded, but the GET request failed."
  exit 0
}

# Extract and display key settings
echo "--- Verification Summary ---"
echo ""

strict=$(echo "${PROTECTION}" | gh api --jq '.required_status_checks.strict // "not set"' --input - 2>/dev/null || echo "parse error")
echo "  Required status checks (strict):     ${strict}"

contexts=$(echo "${PROTECTION}" | gh api --jq '[.required_status_checks.contexts[]] | join(", ")' --input - 2>/dev/null || echo "parse error")
echo "  Required status check contexts:      ${contexts}"

dismiss_stale=$(echo "${PROTECTION}" | gh api --jq '.required_pull_request_reviews.dismiss_stale_reviews // "not set"' --input - 2>/dev/null || echo "parse error")
echo "  Dismiss stale PR reviews:            ${dismiss_stale}"

review_count=$(echo "${PROTECTION}" | gh api --jq '.required_pull_request_reviews.required_approving_review_count // "not set"' --input - 2>/dev/null || echo "parse error")
echo "  Required approving review count:     ${review_count}"

linear=$(echo "${PROTECTION}" | gh api --jq '.required_linear_history.enabled // "not set"' --input - 2>/dev/null || echo "parse error")
echo "  Required linear history:             ${linear}"

force_push=$(echo "${PROTECTION}" | gh api --jq '.allow_force_pushes.enabled // "not set"' --input - 2>/dev/null || echo "parse error")
echo "  Allow force pushes:                  ${force_push}"

deletions=$(echo "${PROTECTION}" | gh api --jq '.allow_deletions.enabled // "not set"' --input - 2>/dev/null || echo "parse error")
echo "  Allow deletions:                     ${deletions}"

enforce_admins=$(echo "${PROTECTION}" | gh api --jq '.enforce_admins.enabled // "not set"' --input - 2>/dev/null || echo "parse error")
echo "  Enforce admins:                      ${enforce_admins}"

echo ""
echo "Branch protection for '${BRANCH}' is configured."
