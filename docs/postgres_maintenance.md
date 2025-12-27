# PostgreSQL maintenance guidance

Large tables (for example `ses.extraction_queue`, `ses.match_results`, `ses.interaction_logs`, `ses.feedback_events`) need periodic maintenance to keep query plans fast and bloat under control. Run the following as a cronjob on the primary:

```
# Nightly: refresh planner stats and reclaim space for heavy-write tables
0 3 * * * psql "$DATABASE_URL" -c "VACUUM ANALYZE ses.extraction_queue; VACUUM ANALYZE ses.match_results; VACUUM ANALYZE ses.interaction_logs; VACUUM ANALYZE ses.feedback_events;"

# Weekly: full analyze to refresh table-wide stats
30 3 * * 0 psql "$DATABASE_URL" -c "ANALYZE VERBOSE ses.extraction_queue; ANALYZE VERBOSE ses.match_results; ANALYZE VERBOSE ses.interaction_logs; ANALYZE VERBOSE ses.feedback_events;"
```

**Notes**
- Prefer running during off-peak hours; adjust schedules to your local traffic patterns.
- If autovacuum is enabled, keep these jobs as an explicit safety net for bursty ingest workloads.
- Consider `VACUUM (VERBOSE)` on especially bloat-prone tables if table bloat grows despite nightly jobs.
