# PostgreSQL メンテナンス指針

`ses.extraction_queue`・`ses.match_results`・`ses.interaction_logs`・`ses.feedback_events` のような大規模テーブルは、統計の鮮度と膨張抑制のために定期メンテが必要です。プライマリで次の cron を回してください。

```
# 毎晩: プランナ統計を更新し、書き込みの多いテーブルの領域を回収
0 3 * * * psql "$DATABASE_URL" -c "VACUUM ANALYZE ses.extraction_queue; VACUUM ANALYZE ses.match_results; VACUUM ANALYZE ses.interaction_logs; VACUUM ANALYZE ses.feedback_events;"

# 週次: 全体統計を取り直し
30 3 * * 0 psql "$DATABASE_URL" -c "ANALYZE VERBOSE ses.extraction_queue; ANALYZE VERBOSE ses.match_results; ANALYZE VERBOSE ses.interaction_logs; ANALYZE VERBOSE ses.feedback_events;"
```

**補足**
- 閑散時間帯に実行する。トラフィックに合わせてスケジュールは調整する。
- autovacuum が有効でも、突発的な大量書き込みに備えた安全網として残す。
- それでも膨張が目立つテーブルには `VACUUM (VERBOSE)` も検討する。
