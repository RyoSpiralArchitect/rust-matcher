#!/usr/bin/env python3
"""
Import talents from Lark CSV export to PostgreSQL.

Usage:
    python scripts/import_talents.py --csv path/to/file.csv --db-url postgres://...

Requirements:
    pip install psycopg2-binary
"""

import argparse
import csv
import os
import re
import sys
from datetime import datetime
from typing import Optional

try:
    import psycopg2
    from psycopg2.extras import execute_values
except ImportError:
    print("Error: psycopg2 not installed. Run: pip install psycopg2-binary")
    sys.exit(1)


def parse_capability(value: str) -> Optional[int]:
    """Convert ◎〇△✕ to numeric (3,2,1,0)."""
    if not value or value.strip() == "":
        return None
    v = value.strip()
    if v in ("◎", "〇〇"):  # Sometimes double circle
        return 3
    if v in ("〇", "○"):
        return 2
    if v == "△":
        return 1
    if v in ("✕", "×", "✗"):
        return 0
    return None


def parse_price(value: str) -> Optional[int]:
    """Parse price like '800000' or '1.05e+06'."""
    if not value or value.strip() == "":
        return None
    try:
        # Handle scientific notation
        return int(float(value.strip()))
    except (ValueError, TypeError):
        return None


def parse_date(value: str) -> Optional[str]:
    """Parse date in various formats."""
    if not value or value.strip() == "":
        return None
    v = value.strip()

    # Try YYYY/MM/DD
    try:
        dt = datetime.strptime(v, "%Y/%m/%d")
        return dt.strftime("%Y-%m-%d")
    except ValueError:
        pass

    # Try YYYY-MM-DD
    try:
        dt = datetime.strptime(v, "%Y-%m-%d")
        return dt.strftime("%Y-%m-%d")
    except ValueError:
        pass

    return None


def parse_array(value: str) -> list[str]:
    """Parse comma-separated values into array."""
    if not value or value.strip() == "":
        return []
    return [v.strip() for v in value.split(",") if v.strip()]


def parse_utilization(value: str) -> Optional[float]:
    """Parse utilization rate (0-1 or 0-100)."""
    if not value or value.strip() == "":
        return None
    try:
        v = float(value.strip())
        # Normalize to 0-1 range
        if v > 1:
            v = v / 100
        return min(1.0, max(0.0, v))
    except (ValueError, TypeError):
        return None


def parse_age(value: str) -> Optional[int]:
    """Parse age."""
    if not value or value.strip() == "":
        return None
    try:
        return int(value.strip())
    except (ValueError, TypeError):
        return None


def parse_birth_year(value: str) -> Optional[int]:
    """Parse birth year."""
    if not value or value.strip() == "":
        return None
    try:
        year = int(value.strip())
        if 1900 <= year <= 2010:
            return year
        return None
    except (ValueError, TypeError):
        return None


def import_csv(csv_path: str, db_url: str, dry_run: bool = False):
    """Import CSV to PostgreSQL."""

    print(f"Reading CSV: {csv_path}")

    rows = []
    with open(csv_path, "r", encoding="utf-8-sig") as f:
        reader = csv.DictReader(f)
        for row in reader:
            rows.append(row)

    print(f"Found {len(rows)} records")

    # Parse rows
    parsed = []
    for i, row in enumerate(rows):
        try:
            record = {
                "name": row.get("氏名", "").strip() or f"Unknown_{i}",
                "age": parse_age(row.get("年齢", "")),
                "birth_year": parse_birth_year(row.get("生年", "")),
                "nearest_station": row.get("最寄り駅", "").strip() or None,
                "desired_price": parse_price(row.get("参考原価", "")),
                "current_utilization": parse_utilization(row.get("現稼働率", "")),
                "available_date": parse_date(row.get("稼働開始可能日", "")),
                "sales_status": row.get("営業状況", "").strip() or None,
                "sales_rep": row.get("担当営業", "").strip() or None,
                "priority_rank": row.get("注力ランク", "").strip() or None,
                "skill_tags": parse_array(row.get("スキルタグ", "")),
                "project_preferences": parse_array(row.get("案件希望", "")),
                "usage_types": parse_array(row.get("用途", "")),
                "capability_pm": parse_capability(row.get("PM/PMO", "")),
                "capability_se": parse_capability(row.get("SE", "")),
                "capability_bpo": parse_capability(row.get("BPO", "")),
                "capability_consul": parse_capability(row.get("コンサル", "")),
                "english_level": row.get("英語", "").strip() or None,
                "business_relationship": row.get("SPONTOから見る商流", "").strip() or None,
                "phone": row.get("携帯番号", "").strip() or None,
                "email": row.get("メアド", "").strip() or None,
                "linkedin_url": row.get("Linkedin", "").strip() or None,
                "skill_sheet_url": row.get("スキルシート", "").strip() or None,
                "skill_sheet_url_2": row.get("スキルシート２", "").strip() or None,
                "memo": row.get("メモ", "").strip() or None,
                "inflow_source": row.get("流入経路", "").strip() or None,
                "inflow_date": parse_date(row.get("流入日", "")),
                "registration_date": parse_date(row.get("登録日", "")),
            }
            parsed.append(record)
        except Exception as e:
            print(f"Warning: Failed to parse row {i}: {e}")
            continue

    print(f"Parsed {len(parsed)} records successfully")

    # Sample output
    if parsed:
        sample = parsed[0]
        print(f"\nSample record:")
        print(f"  Name: {sample['name']}")
        print(f"  Age: {sample['age']}")
        print(f"  Price: {sample['desired_price']}")
        print(f"  Skills: {sample['skill_tags'][:3]}...")
        print(f"  Status: {sample['sales_status']}")

    if dry_run:
        print("\n[DRY RUN] Would insert {len(parsed)} records")
        return

    # Insert to PostgreSQL
    print(f"\nConnecting to database...")
    conn = psycopg2.connect(db_url)
    cur = conn.cursor()

    # Create table if not exists (use the DDL from schema.rs)
    print("Ensuring ses.talents table exists...")
    cur.execute("""
        CREATE TABLE IF NOT EXISTS ses.talents (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            age INTEGER,
            birth_year INTEGER,
            nearest_station TEXT,
            desired_price INTEGER,
            current_utilization REAL,
            available_date DATE,
            sales_status TEXT,
            sales_rep TEXT,
            priority_rank TEXT,
            skill_tags TEXT[],
            project_preferences TEXT[],
            usage_types TEXT[],
            capability_pm INTEGER,
            capability_se INTEGER,
            capability_bpo INTEGER,
            capability_consul INTEGER,
            english_level TEXT,
            business_relationship TEXT,
            phone TEXT,
            email TEXT,
            linkedin_url TEXT,
            skill_sheet_url TEXT,
            skill_sheet_url_2 TEXT,
            memo TEXT,
            inflow_source TEXT,
            inflow_date DATE,
            registration_date DATE,
            lark_record_id TEXT UNIQUE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
        );
    """)

    # Create indexes if not exist
    cur.execute("CREATE INDEX IF NOT EXISTS idx_talents_name ON ses.talents(name);")
    cur.execute("CREATE INDEX IF NOT EXISTS idx_talents_sales_status ON ses.talents(sales_status);")

    # Insert data
    print(f"Inserting {len(parsed)} records...")

    columns = [
        "name", "age", "birth_year", "nearest_station",
        "desired_price", "current_utilization", "available_date",
        "sales_status", "sales_rep", "priority_rank",
        "skill_tags", "project_preferences", "usage_types",
        "capability_pm", "capability_se", "capability_bpo", "capability_consul",
        "english_level", "business_relationship",
        "phone", "email", "linkedin_url",
        "skill_sheet_url", "skill_sheet_url_2",
        "memo", "inflow_source", "inflow_date", "registration_date"
    ]

    values = [
        tuple(record.get(col) for col in columns)
        for record in parsed
    ]

    # Use UPSERT pattern based on name (since we don't have lark_record_id yet)
    insert_sql = f"""
        INSERT INTO ses.talents ({', '.join(columns)})
        VALUES %s
        ON CONFLICT DO NOTHING
    """

    execute_values(cur, insert_sql, values, page_size=100)

    conn.commit()

    # Verify
    cur.execute("SELECT COUNT(*) FROM ses.talents")
    count = cur.fetchone()[0]
    print(f"\nDone! Total records in ses.talents: {count}")

    cur.close()
    conn.close()


def main():
    parser = argparse.ArgumentParser(description="Import talents from Lark CSV")
    parser.add_argument("--csv", required=True, help="Path to CSV file")
    parser.add_argument("--db-url", required=False, help="PostgreSQL connection URL")
    parser.add_argument("--dry-run", action="store_true", help="Parse only, don't insert")
    args = parser.parse_args()

    db_url = args.db_url or os.environ.get("DATABASE_URL")
    if not db_url and not args.dry_run:
        print("Error: --db-url or DATABASE_URL required")
        sys.exit(1)

    import_csv(args.csv, db_url, args.dry_run)


if __name__ == "__main__":
    main()
