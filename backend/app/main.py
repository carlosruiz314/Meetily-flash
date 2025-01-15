from fastapi import FastAPI, WebSocket
from fastapi.middleware.cors import CORSMiddleware
import asyncio
import json
from typing import Optional, List, Dict
from pydantic import BaseModel

app = FastAPI()

# Configure CORS
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],  # In production, replace with specific origins
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

class Block(BaseModel):
    id: str
    type: str
    content: str
    color: str

class Section(BaseModel):
    title: str
    blocks: List[Block]

class SummaryResponse(BaseModel):
    podcastOverview: Section
    sourcingInsights: Section
    investmentChallenges: Section
    perspectivesOnVC: Section
    personalReflections: Section
    closingRemarks: Section

class ProcessRequest(BaseModel):
    transcript: str
    metadata: Optional[Dict] = None

@app.post("/process")
async def process_transcript(request: ProcessRequest) -> SummaryResponse:
    # This is a mock response - replace with actual AI processing
    return SummaryResponse(
        podcastOverview=Section(
            title="Discussion Highlights",
            blocks=[
                Block(id="1", type="bullet", content="Exploration of venture capital (VC) sourcing strategies and challenges.", color="default"),
                Block(id="2", type="bullet", content="Insights on investment risk and the role of accredited investors.", color="gray"),
                Block(id="3", type="bullet", content="Discussion on the evolution and specialization of VC practices.", color="default")
            ]
        ),
        sourcingInsights=Section(
            title="How VC Firms Source Investments",
            blocks=[
                Block(id="4", type="bullet", content="Companies often found through public announcements, events, and conferences.", color="default"),
                Block(id="5", type="bullet", content="Venture arms play a key role in identifying startups seeking funding.", color="gray"),
                Block(id="6", type="bullet", content="Networking at industry forums and academic gatherings is crucial.", color="default")
            ]
        ),
        investmentChallenges=Section(
            title="Challenges in Venture Investments",
            blocks=[
                Block(id="7", type="bullet", content="High-risk business with only 1 out of 100 investments typically succeeding.", color="default"),
                Block(id="8", type="bullet", content="Success rates vary significantly between established and new VC firms.", color="gray"),
                Block(id="9", type="bullet", content="Equity investments can result in total loss if the company fails.", color="default")
            ]
        ),
        perspectivesOnVC=Section(
            title="Perspectives on VC Practices",
            blocks=[
                Block(id="10", type="bullet", content="Comparison of VCs to the evolution of surgeons highlights growth potential.", color="default"),
                Block(id="11", type="bullet", content="Role of trust in the people behind startups as a key investment factor.", color="gray"),
                Block(id="12", type="bullet", content="Challenges in maintaining value and protecting investments discussed.", color="default")
            ]
        ),
        personalReflections=Section(
            title="Personal Reflections and Insights",
            blocks=[
                Block(id="13", type="bullet", content="Importance of the ability to get things done highlighted.", color="default"),
                Block(id="14", type="bullet", content="Inspirations from Steve Jobs and his approach to innovation.", color="gray"),
                Block(id="15", type="bullet", content="Reflections on the significance of academic and early professional experiences.", color="default")
            ]
        ),
        closingRemarks=Section(
            title="Closing Remarks",
            blocks=[
                Block(id="16", type="bullet", content="Appreciation for the conversation and insights shared.", color="default"),
                Block(id="17", type="bullet", content="Emphasis on the lasting impact of early professional relationships.", color="gray"),
                Block(id="18", type="bullet", content="Host expresses gratitude for the guests participation.", color="default")
            ]
        )
    )

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="127.0.0.1", port=5167)
