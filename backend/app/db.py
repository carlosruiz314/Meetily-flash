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
                    error TEXT,
                    chunk_count INTEGER DEFAULT 0,
                    processing_time REAL DEFAULT 0.0,
                    metadata TEXT
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

    async def update_process(self, process_id: str, status: str, result: Optional[Dict] = None, error: Optional[str] = None, 
                           chunk_count: Optional[int] = None, processing_time: Optional[float] = None, 
                           metadata: Optional[Dict] = None):
        """Update a process status and result"""
        now = datetime.utcnow().isoformat()
        
        async with self._get_connection() as conn:
            update_fields = ["status = ?", "updated_at = ?"]
            params = [status, now]
            
            if result:
                update_fields.append("result = ?")
                params.append(json.dumps(result))
            if error:
                update_fields.append("error = ?")
                params.append(error)
            if chunk_count is not None:
                update_fields.append("chunk_count = ?")
                params.append(chunk_count)
            if processing_time is not None:
                update_fields.append("processing_time = ?")
                params.append(processing_time)
            if metadata:
                update_fields.append("metadata = ?")
                params.append(json.dumps(metadata))
                
            params.append(process_id)
            query = f"UPDATE summary_processes SET {', '.join(update_fields)} WHERE id = ?"
            await conn.execute(query, params)
            await conn.commit()

    async def get_process(self, process_id: str) -> Optional[Dict[str, Any]]:
        """Get a process by its ID"""
        async with self._get_connection() as conn:
            async with conn.execute(
                "SELECT id, status, created_at, updated_at, result, error, chunk_count, processing_time, metadata FROM summary_processes WHERE id = ?",
                (process_id,)
            ) as cursor:
                row = await cursor.fetchone()
                
                if not row:
                    return None
                    
                result = {
                    "id": row[0],
                    "status": row[1],
                    "created_at": row[2],
                    "updated_at": row[3],
                    "chunk_count": row[6],
                    "processing_time": row[7]
                }
                
                if row[4]:  # result
                    result["result"] = json.loads(row[4])
                if row[5]:  # error
                    result["error"] = row[5]
                if row[8]:  # metadata
                    result["metadata"] = json.loads(row[8])
                    
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
