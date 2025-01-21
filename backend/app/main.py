from fastapi import FastAPI, HTTPException, BackgroundTasks, UploadFile, File
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse
from pydantic import BaseModel
import uvicorn
from typing import Optional, Dict, Any, List
import logging
from datetime import datetime
import os
from dotenv import load_dotenv
from db import DatabaseManager
import asyncio
from functools import partial
import json
from threading import Lock
from Anthropic_transcript import (
    TranscriptProcessor, MeetingSummarizer, SummaryResponse,
    SYSTEM_PROMPT, Agent, RunContext
)
import uuid

# Load environment variables
load_dotenv()

# Configure logger with line numbers and function names
logger = logging.getLogger(__name__)
logger.setLevel(logging.DEBUG)

# Create console handler with formatting
console_handler = logging.StreamHandler()
console_handler.setLevel(logging.DEBUG)

# Create formatter with line numbers and function names
formatter = logging.Formatter(
    '%(asctime)s - %(levelname)s - [%(filename)s:%(lineno)d - %(funcName)s()] - %(message)s',
    datefmt='%Y-%m-%d %H:%M:%S'
)
console_handler.setFormatter(formatter)

# Add handler to logger if not already added
if not logger.handlers:
    logger.addHandler(console_handler)

app = FastAPI(
    title="Meeting Summarizer API",
    description="API for processing and summarizing meeting transcripts",
    version="1.0.0"
)

# Configure CORS
app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:3118"],  # Frontend URL
    allow_credentials=True,
    allow_methods=["GET", "POST", "OPTIONS"],
    allow_headers=["*"],
    max_age=3600,  # Cache preflight requests for 1 hour
)


class TranscriptRequest(BaseModel):
    """Request model for transcript text"""
    text: str
    chunk_size: Optional[int] = 5000
    overlap: Optional[int] = 1000


class TranscriptResponse(BaseModel):
    """Response model for transcript processing"""
    message: str
    num_chunks: int
    data: Dict[str, Any]


class SummaryProcessor:
    """Handles the processing of summaries in a thread-safe way"""
    def __init__(self):
        try:
            self.db = DatabaseManager()
            self._lock = Lock()
            
            # Load API key and validate
            api_key = os.getenv('ANTHROPIC_API_KEY')
            if not api_key:
                logger.error("ANTHROPIC_API_KEY environment variable not set")
                raise ValueError("ANTHROPIC_API_KEY environment variable not set")
            
            logger.info("Initializing SummaryProcessor components")
            self.transcript_processor = TranscriptProcessor()
            self.summarizer = MeetingSummarizer(api_key)
            self.agent = Agent(
                model=self.summarizer.model,
                system_prompt=SYSTEM_PROMPT
            )
            self.collection = None
            self.final_summary_result = None
            logger.info("SummaryProcessor initialized successfully")
        except Exception as e:
            logger.error(f"Failed to initialize SummaryProcessor: {str(e)}", exc_info=True)
            raise

    async def process_transcript(self, text: str, chunk_size: int = 5000, overlap: int = 1000) -> tuple:
        """Process a transcript text"""
        try:
            if not text:
                raise ValueError("Empty transcript text provided")
            
            # Validate chunk_size and overlap
            if chunk_size <= 0:
                raise ValueError("chunk_size must be positive")
            if overlap < 0:
                raise ValueError("overlap must be non-negative")
            if overlap >= chunk_size:
                overlap = chunk_size - 1  # Ensure overlap is less than chunk_size
            
            # Ensure step size is positive
            step_size = chunk_size - overlap
            if step_size <= 0:
                chunk_size = overlap + 1  # Adjust chunk_size to ensure positive step
                
            logger.info("Initializing ChromaDB collection")
            self.transcript_processor.initialize_collection()
            self.collection = self.transcript_processor.collection
            
            if not self.collection:
                raise ValueError("Failed to initialize ChromaDB collection")
            
            logger.info(f"Processing transcript of length {len(text)} with chunk_size={chunk_size}, overlap={overlap}")
            # Pass text as positional arg, chunk_size and overlap as keyword args
            num_chunks, all_json_data = await self.transcript_processor.process_transcript(
                text=text,  # Pass as keyword arg to be explicit
                chunk_size=chunk_size,
                overlap=overlap
            )
            logger.info(f"Successfully processed transcript into {num_chunks} chunks")
            
            return num_chunks, all_json_data
        except Exception as e:
            logger.error(f"Error processing transcript: {str(e)}", exc_info=True)
            raise

    async def process_summary(self, process_id: str) -> Dict[str, Any]:
        """Process a summary in a thread-safe way"""
        try:
            logger.info(f"Processing summary for process {process_id}")
            if not self.collection:
                logger.info("Initializing ChromaDB collection")
                self.transcript_processor.initialize_collection()
                self.collection = self.transcript_processor.collection
                
                if not self.collection:
                    raise ValueError("Failed to initialize ChromaDB collection")
            
            # Run the agent to generate summary
            logger.info("Running agent to generate summary")
            try:
                run_result = await self.agent.run(
                    'What is the summary of the following meeting? Use tools to get the data'
                )
                logger.info("Successfully received model response")
                
                if not run_result or not hasattr(run_result, 'data'):
                    raise ValueError("Invalid response from agent: missing data")
                
                total_summary_in_pydantic = self.summarizer.generate_summary(run_result.data)
                
                # Validate summary has content
                if not any([
                    total_summary_in_pydantic.Agenda.blocks,
                    total_summary_in_pydantic.Decisions.blocks,
                    total_summary_in_pydantic.ActionItems.blocks,
                    total_summary_in_pydantic.ClosingRemarks.blocks
                ]):
                    raise ValueError("No content found in summary")

                # Convert to JSON using Pydantic's json() method
                json_data = total_summary_in_pydantic.model_dump_json(indent=2)
                raw_summary = json.loads(json_data)

                # Format the summary to match the frontend Summary type
                formatted_summary = {}
                for section_name, section_data in raw_summary.items():
                    formatted_blocks = []
                    for block in section_data.get('blocks', []):
                        formatted_block = {
                            'id': block.get('id', str(uuid.uuid4())),
                            'type': block.get('type', 'text'),  # Default to 'text' if not specified
                            'content': block.get('content', ''),
                            'color': block.get('color', 'default')
                        }
                        formatted_blocks.append(formatted_block)

                    formatted_summary[section_name] = {
                        'title': section_data.get('title', section_name),
                        'blocks': formatted_blocks
                    }

                # Return the result with usage information
                result = {
                    "summary": formatted_summary,
                    "usage": {
                        "requests": run_result._usage.requests if hasattr(run_result, '_usage') else 0,
                        "request_tokens": run_result._usage.request_tokens if hasattr(run_result, '_usage') else 0,
                        "response_tokens": run_result._usage.response_tokens if hasattr(run_result, '_usage') else 0,
                        "total_tokens": run_result._usage.total_tokens if hasattr(run_result, '_usage') else 0
                    }
                }
                
                logger.info(f"Successfully generated summary for process {process_id}")
                return result
                
            except Exception as e:
                logger.error(f"Error running agent: {str(e)}", exc_info=True)
                if "empty model response" in str(e).lower():
                    raise ValueError("Empty model response received. This may indicate an issue with the API key or model configuration.")
                raise
            
        except Exception as e:
            logger.error(f"Error processing summary: {str(e)}", exc_info=True)
            raise

    def cleanup(self):
        """Cleanup resources"""
        try:
            logger.info("Cleaning up resources")
            self.transcript_processor.cleanup()
            logger.info("Cleanup completed successfully")
        except Exception as e:
            logger.error(f"Error during cleanup: {str(e)}", exc_info=True)


# Initialize processor
processor = SummaryProcessor()

# Define tools
@processor.agent.tool
async def query_transcript(ctx: RunContext, query: str) -> str:
    """Query the transcript to extract information. Returns the content and chunk IDs for deletion."""
    try:
        logger.info(f"Querying transcript with: {query}")
        
        # Check if there are any chunks left
        if not processor.collection:
            logger.error("No ChromaDB collection available")
            return "Error: No transcript loaded. Please process a transcript first."
            
        collection_data = processor.collection.get()
        if not collection_data['ids']:
            logger.info("No chunks left to process")
            return "CHROMADB_EMPTY: All chunks have been processed."
            
        # Get unprocessed chunks
        logger.info("Querying ChromaDB for relevant chunks")
        results = processor.collection.query(
            query_texts=[query],
            n_results=1
        )
        
        if not results or not results['documents'] or not results['documents'][0]:
            logger.info("No results found for query")
            return "No results found for the query"
            
        # Process and immediately delete chunks
        combined_result = ""
        chunk_ids = []
        
        for doc, metadata, id in zip(results['documents'][0], results['metadatas'][0], results['ids'][0]):
            combined_result += f"\n{doc}\n"
            chunk_ids.append(id)
        
        # Delete the chunks we just processed
        if chunk_ids:
            try:
                logger.info(f"Deleting {len(chunk_ids)} processed chunks")
                processor.collection.delete(ids=chunk_ids)
                
                # Verify deletion
                remaining = processor.collection.get()
                logger.info(f"Remaining chunks: {len(remaining['ids'])}")
                
            except Exception as e:
                logger.error(f"Error deleting chunks: {str(e)}", exc_info=True)
                return f"Error deleting chunks: {str(e)}"
            
        return combined_result.strip()
        
    except Exception as e:
        logger.error(f"Error querying transcript: {str(e)}", exc_info=True)
        return f"Error: {str(e)}"

@processor.agent.tool
async def delete_processed_chunks(ctx: RunContext) -> str:
    """Delete all processed chunks from the collection"""
    try:
        if not hasattr(ctx, 'processed_chunks') or not ctx.processed_chunks:
            return "No chunks to delete"
            
        chunk_ids = list(ctx.processed_chunks)
        processor.collection.delete(ids=chunk_ids)
        
        # Clear the processed chunks
        ctx.processed_chunks.clear()
        
        return f"Successfully deleted {len(chunk_ids)} chunks"
        
    except Exception as e:
        logger.error(f"Error deleting chunks: {e}")
        return f"Error deleting chunks: {str(e)}"

@processor.agent.tool
async def add_action_item(ctx: RunContext, title: str, content: str) -> str:
    """Add an action item to the summary"""
    try:
        logger.info(f"Adding action item: {title}")
        result = processor.summarizer.add_action_item(ctx, title, content)
        logger.info("Successfully added action item")
        return f"Successfully added action item: {result}"
    except Exception as e:
        logger.error(f"Error adding action item: {str(e)}", exc_info=True)
        return f"Error adding action item: {str(e)}"

@processor.agent.tool
async def add_agenda_item(ctx: RunContext, title: str, content: str) -> str:
    """Add an agenda item to the summary"""
    try:
        logger.info(f"Adding agenda item: {title}")
        result = processor.summarizer.add_agenda_item(ctx, title, content)
        logger.info("Successfully added agenda item")
        return f"Successfully added agenda item: {result}"
    except Exception as e:
        logger.error(f"Error adding agenda item: {str(e)}", exc_info=True)
        return f"Error adding agenda item: {str(e)}"

@processor.agent.tool
async def add_decision(ctx: RunContext, title: str, content: str) -> str:
    """Add a decision to the summary"""
    try:
        logger.info(f"Adding decision: {title}")
        result = processor.summarizer.add_decision(ctx, title, content)
        logger.info("Successfully added decision")
        return f"Successfully added decision: {result}"
    except Exception as e:
        logger.error(f"Error adding decision: {str(e)}", exc_info=True)
        return f"Error adding decision: {str(e)}"

@processor.agent.tool
async def save_final_summary_result(ctx: RunContext) -> str:
    """
    Save the final meeting summary result to a file
    args:
        ctx (RunContext): The run context

    returns:    
        str: Status message indicating success or failure
    """
    try:
        # Get the final summary result
        summary = processor.summarizer.generate_summary(ctx)
        
        # Validate summary has content
        if not any([
            summary.Agenda.blocks,
            summary.Decisions.blocks,
            summary.ActionItems.blocks,
            summary.ClosingRemarks.blocks
        ]):
            return "Error: No content found in summary. Please add some items first."

        # Convert to JSON using Pydantic's json() method which handles nested models
        json_data = summary.model_dump_json(indent=2)

        self.final_summary_result = json_data
        
        # Save to file with error handling
        try:
            with open('final_summary_result.json', 'w') as f:
                f.write(json_data)
            return "Successfully saved final summary result to file"
        except IOError as e:
            logger.error(f"Failed to write summary to file: {e}")
            return f"Error saving to file: {str(e)}"
            
    except Exception as e:
        logger.error(f"Error generating or saving summary: {e}")
        return f"Error processing summary: {str(e)}"

@processor.agent.tool
async def get_final_summary(ctx: RunContext) -> SummaryResponse:
    """Get the final meeting summary result"""
    try:
        logger.info("Generating final summary")
        summary = processor.summarizer.generate_summary(ctx)
        logger.info("Successfully generated final summary")
        return summary
    except Exception as e:
        logger.error(f"Error generating final summary: {str(e)}", exc_info=True)
        raise

@app.post("/process-transcript", response_model=TranscriptResponse)
async def process_transcript(transcript: TranscriptRequest) -> TranscriptResponse:
    """Process a transcript text"""
    logger.info("Received request to process transcript")
    try:
        # Process the transcript
        logger.info(f"Processing transcript with chunk_size={transcript.chunk_size}, overlap={transcript.overlap}")
        logger.info(f"Transcript text: {len(transcript.text)} characters")
        num_chunks, all_json_data = await processor.process_transcript(
            transcript.text,
            chunk_size=transcript.chunk_size,
            overlap=transcript.overlap
        )
        logger.info(f"Successfully processed transcript into {num_chunks} chunks")
        
        # Convert all_json_data to list of dictionaries if it's not already
        if isinstance(all_json_data, str):
            json_data = json.loads(all_json_data)
        elif isinstance(all_json_data, list):
            json_data = all_json_data
        else:
            json_data = [all_json_data] if all_json_data else []
        final_summary = {
            "Agenda": {
                "title": "Agenda",
                "blocks": []
            },
            "Decisions": {
                "title": "Decisions",
                "blocks": []
            },
            "ActionItems": {
                "title": "Action Items",
                "blocks": []
            },
            "ClosingRemarks": {
                "title": "Closing Remarks",
                "blocks": []
            }
        }
        # Add all the blocks from all the JSON objects to the new JSON object
        for json_obj in all_json_data:
            logger.info(f"Processing JSON object {json_obj}, type {type(json_obj)}")
            # Convert the JSON object to a dictionary
            json_dict = json.loads(json_obj)
            # Add the blocks from the dictionary to the new JSON object
            final_summary["Agenda"]["blocks"].extend(json_dict["Agenda"]["blocks"])
            final_summary["Decisions"]["blocks"].extend(json_dict["Decisions"]["blocks"])
            final_summary["ActionItems"]["blocks"].extend(json_dict["ActionItems"]["blocks"])
            final_summary["ClosingRemarks"]["blocks"].extend(json_dict["ClosingRemarks"]["blocks"])
        
        return TranscriptResponse(
            message="Successfully processed transcript",
            num_chunks=num_chunks,
            data=final_summary
        )
    except Exception as e:
        logger.error(f"Error processing transcript: {str(e)}", exc_info=True)
        raise HTTPException(status_code=500, detail=str(e))

@app.post("/upload-transcript")
async def upload_transcript(file: UploadFile = File(...)) -> Dict[str, str]:
    """Upload and process a transcript file"""
    logger.info(f"Received transcript file upload: {file.filename}")
    try:
        content = await file.read()
        transcript_text = content.decode()
        logger.info("Successfully decoded transcript file content")
        
        num_chunks = await processor.process_transcript(
            transcript_text,
            chunk_size=5000,
            overlap=1000
        )
        logger.info(f"Successfully processed transcript file into {num_chunks} chunks")
        return {"message": f"Successfully processed transcript file into {num_chunks} chunks"}
    except Exception as e:
        logger.error(f"Error processing transcript file: {e}", exc_info=True)
        raise HTTPException(status_code=500, detail=str(e))

@app.post("/start-summarization")
async def start_summarization(background_tasks: BackgroundTasks) -> Dict[str, str]:
    """Start the summarization process"""
    logger.info("Received request to start summarization")
    try:
        # Create a new process in the database
        process_id = await processor.db.create_process()
        logger.info(f"Created process with ID: {process_id}")
        
        # Start background processing
        background_tasks.add_task(process_and_update, process_id)
        logger.info(f"Added background task for process {process_id}")
        
        return {"process_id": process_id}
    except Exception as e:
        logger.error(f"Error in start_summarization: {str(e)}", exc_info=True)
        raise HTTPException(status_code=500, detail=str(e))

async def process_and_update(process_id: str):
    """Process the summary and update the database"""
    logger.info(f"Starting background processing for {process_id}")
    try:
        # Process the summary
        result = await processor.process_summary(process_id)
        logger.info(f"Generated summary for {process_id}")
        
        # Update the database with the result
        await processor.db.update_process(process_id, "COMPLETED", result=result)
        logger.info(f"Updated process {process_id} as completed")
    except Exception as e:
        logger.error(f"Error in process_and_update for {process_id}: {str(e)}", exc_info=True)
        await processor.db.update_process(process_id, "FAILED", error=str(e))

@app.get("/get-summary/{process_id}")
async def get_summary(process_id: str) -> Dict[str, Any]:
    """Get the summary for a specific process"""
    logger.info(f"Received request to get summary for process {process_id}")
    try:
        process = await processor.db.get_process(process_id)
        if not process:
            logger.warning(f"Process not found for ID {process_id}")
            raise HTTPException(status_code=404, detail="Process not found")
        
        if process.get("error"):
            logger.error(f"Process {process_id} failed with error: {process['error']}")
            raise HTTPException(status_code=500, detail=process["error"])
            
        if process.get("status") != "COMPLETED":
            logger.info(f"Process {process_id} is still {process.get('status')}")
            return {"status": process.get("status")}
            
        if not process.get("result"):
            logger.warning(f"No result found for completed process {process_id}")
            raise HTTPException(status_code=404, detail="Summary not found")
            
        logger.info(f"Successfully retrieved summary for process {process_id}")
        return process["result"]
    except Exception as e:
        logger.error(f"Error retrieving summary: {str(e)}", exc_info=True)
        raise HTTPException(status_code=500, detail=str(e))

@app.on_event("shutdown")
async def shutdown_event():
    """Cleanup on API shutdown"""
    logger.info("API shutting down, cleaning up resources")
    try:
        await processor.cleanup()
        logger.info("Successfully cleaned up resources")
    except Exception as e:
        logger.error(f"Error during cleanup: {str(e)}", exc_info=True)

if __name__ == "__main__":
    import multiprocessing
    multiprocessing.freeze_support()
    uvicorn.run(app, host="0.0.0.0", port=5167)
