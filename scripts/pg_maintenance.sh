#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${DATABASE_URL:-}" ]]; then
  echo "DATABASE_URL is required"
  exit 1
fi

echo "[pg-maintenance] $(date -u +"%Y-%m-%dT%H:%M:%SZ") start"

psql "${DATABASE_URL}" -v ON_ERROR_STOP=1 <<'SQL'
VACUUM ANALYZE ses.extraction_queue;
VACUUM ANALYZE ses.match_results;
VACUUM ANALYZE ses.interaction_logs;
VACUUM ANALYZE ses.feedback_events;

ANALYZE VERBOSE ses.extraction_queue;
ANALYZE VERBOSE ses.match_results;
ANALYZE VERBOSE ses.interaction_logs;
ANALYZE VERBOSE ses.feedback_events;
SQL

echo "[pg-maintenance] $(date -u +"%Y-%m-%dT%H:%M:%SZ") done"
