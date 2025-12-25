#!/usr/bin/env python3
"""
Import talents from Lark CSV into ses.talents_enum for GUI testing.
"""
import csv
import os
import sys
import psycopg2
from datetime import datetime
import uuid

def parse_date(date_str):
    """Parse Japanese date format YYYY/MM/DD"""
    if not date_str or date_str.strip() == '':
        return None
    try:
        return datetime.strptime(date_str.strip(), '%Y/%m/%d').date()
    except ValueError:
        return None

def parse_price(price_str):
    """Parse price string to integer"""
    if not price_str or price_str.strip() == '':
        return None
    try:
        return int(price_str.replace(',', '').strip())
    except ValueError:
        return None

def main():
    csv_path = sys.argv[1] if len(sys.argv) > 1 else '/Users/ryospiralarchitect/SpiralReality/BP・要員管理_要員管理_要員一覧.csv'

    database_url = os.environ.get('DATABASE_URL')
    if not database_url:
        print("ERROR: DATABASE_URL not set")
        sys.exit(1)

    conn = psycopg2.connect(database_url)
    cur = conn.cursor()

    # Read CSV (with BOM handling)
    with open(csv_path, 'r', encoding='utf-8-sig') as f:
        reader = csv.DictReader(f)
        rows = list(reader)

    print(f"Found {len(rows)} rows in CSV")

    inserted = 0
    for i, row in enumerate(rows):
        name = row.get('氏名', '').strip()
        if not name:
            continue

        # Generate unique message_id
        message_id = f"lark-talent-{i+1}-{uuid.uuid4().hex[:8]}"

        talent_name = name
        desired_price_min = parse_price(row.get('参考原価', ''))
        available_date = parse_date(row.get('稼働開始可能日', ''))

        # Build summary from skill tags and preferences
        skill_tags = row.get('スキルタグ', '')
        preferences = row.get('案件希望', '')
        summary_parts = []
        if skill_tags:
            summary_parts.append(f"Skills: {skill_tags}")
        if preferences:
            summary_parts.append(f"Preferences: {preferences}")
        summary_text = ' | '.join(summary_parts) if summary_parts else None

        try:
            cur.execute("""
                INSERT INTO ses.talents_enum (message_id, talent_name, summary_text, desired_price_min, available_date)
                VALUES (%s, %s, %s, %s, %s)
                ON CONFLICT (message_id) DO NOTHING
            """, (message_id, talent_name, summary_text, desired_price_min, available_date))
            inserted += 1
        except Exception as e:
            print(f"Error inserting {name}: {e}")

    conn.commit()
    print(f"Inserted {inserted} talents into ses.talents_enum")

    # Also create extraction_queue entries for testing
    cur.execute("SELECT id, message_id, talent_name FROM ses.talents_enum LIMIT 10")
    talents = cur.fetchall()

    queue_inserted = 0
    for talent_id, message_id, talent_name in talents:
        try:
            cur.execute("""
                INSERT INTO ses.extraction_queue (message_id, email_subject, email_received_at, status, priority)
                VALUES (%s, %s, NOW(), 'completed', 50)
                ON CONFLICT (message_id) DO NOTHING
            """, (message_id, f"Talent: {talent_name}"))
            queue_inserted += 1
        except Exception as e:
            print(f"Error creating queue entry: {e}")

    conn.commit()
    print(f"Created {queue_inserted} extraction_queue entries")

    cur.close()
    conn.close()
    print("Done!")

if __name__ == '__main__':
    main()
