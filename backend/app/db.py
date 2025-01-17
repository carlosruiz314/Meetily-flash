import aiosqlite
import json
from datetime import datetime, timedelta
import uuid
from typing import Optional, Dict, Any
import logging
import asyncio
from contextlib import asynccontextmanager

logger = logging.getLogger(__name__)

class DatabaseManager:
    def __init__(self, db_path: str = "summaries.db"):
        self.db_path = db_path
        self._init_db()

    def _init_db(self):
        """Initialize the database with required tables"""
        import sqlite3  # Use sync sqlite3 for initialization only
        with sqlite3.connect(self.db_path) as conn:
            cursor = conn.cursor()
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS summary_processes (
                    id TEXT PRIMARY KEY,
                    status TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    result TEXT,
                    error TEXT
                )
            """)
            conn.commit()

    @asynccontextmanager
    async def _get_connection(self):
        """Get a new database connection"""
        conn = await aiosqlite.connect(self.db_path)
        try:
            yield conn
        finally:
            await conn.close()

    async def create_process(self) -> str:
        """Create a new process entry and return its ID"""
        process_id = str(uuid.uuid4())
        now = datetime.utcnow().isoformat()
        
        async with self._get_connection() as conn:
            await conn.execute(
                "INSERT INTO summary_processes (id, status, created_at, updated_at) VALUES (?, ?, ?, ?)",
                (process_id, "PENDING", now, now)
            )
            await conn.commit()
        
        return process_id

    async def update_process(self, process_id: str, status: str, result: Optional[Dict] = None, error: Optional[str] = None):
        """Update a process status and result"""
        now = datetime.utcnow().isoformat()
        
        async with self._get_connection() as conn:
            if result:
                result_json = json.dumps(result)
                await conn.execute(
                    "UPDATE summary_processes SET status = ?, updated_at = ?, result = ? WHERE id = ?",
                    (status, now, result_json, process_id)
                )
            elif error:
                await conn.execute(
                    "UPDATE summary_processes SET status = ?, updated_at = ?, error = ? WHERE id = ?",
                    (status, now, error, process_id)
                )
            else:
                await conn.execute(
                    "UPDATE summary_processes SET status = ?, updated_at = ? WHERE id = ?",
                    (status, now, process_id)
                )
            await conn.commit()

    async def get_process(self, process_id: str) -> Optional[Dict[str, Any]]:
        """Get a process by its ID"""
        async with self._get_connection() as conn:
            async with conn.execute(
                "SELECT id, status, created_at, updated_at, result, error FROM summary_processes WHERE id = ?",
                (process_id,)
            ) as cursor:
                row = await cursor.fetchone()
                
                if not row:
                    return None
                    
                result = {
                    "id": row[0],
                    "status": row[1],
                    "created_at": row[2],
                    "updated_at": row[3]
                }
                
                if row[4]:  # result
                    result["result"] = json.loads(row[4])
                if row[5]:  # error
                    result["error"] = row[5]
                    
                return result

    async def cleanup_old_processes(self, hours: int = 24):
        """Clean up processes older than specified hours"""
        cutoff = (datetime.utcnow() - timedelta(hours=hours)).isoformat()
        
        async with self._get_connection() as conn:
            await conn.execute(
                "DELETE FROM summary_processes WHERE created_at < ?",
                (cutoff,)
            )
            await conn.commit()
