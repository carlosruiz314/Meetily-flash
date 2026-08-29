# Hybrid engine transcript preview — meeting cde5c264

> **HISTORICAL SIMULATION ARTIFACT — NOT current engine output.** Produced by the
> pre-amendment simulation (`hybrid_engine_sim`, commit 9a39909). It still shows
> the known defects the `hybrid-diarization-engine` change fixes: the false flip
> at 02:39 (real change ≈163s) and the invalid `crosstalk 83%` flag
> (max-of-pieces bug; raw frames show ≈2.5%). Do not review attribution from
> this file. The review artifact is produced fresh at task 5.4 of the change.
> Text garbage below is whisper output — out of scope for that change; review
> attribution only.

Turns = pyannote speech-runs (pause-delimited); labels = TitaNet run-level
embeddings mapped to DB cluster names. `crosstalk NN%` = fraction of frames
where pyannote detects a second simultaneous voice — identity there is
genuinely ambiguous for any local method. A trailing `…` marks a sentence
that continues into the next turn (real overlap/interruption). Textless
voiced runs (breaths/laughs) are dropped before coalescing.

**[00:01–00:30] Speaker 0**  `crosstalk 75%`

  How's it going? All good, all good. You? I don't like you've aged like five years. Yeah. That's right. Oh, man Okay. I have some updates. Cool. On the roadmap, hopefully. Okay. Let's go. So for search, do we want to, let's, wait, we've got to record this

**[00:30–00:52] Speaker 1**

  Yeah, sure, sure, sure. Yeah, for Paul ina, right? Where is Ricardo I don't know. Let me ping in. I can't. I can't. I saw you there in the meeting room alone and I was like, damn. Oh, you're wearing the t-shirt. Yeah.

**[00:57–01:08] Speaker 0**  `…`

  Okay, we can start. He's coming in too. Okay. I'll start with search. So we didn't get to search XP. Fair. We'll have motor stuff regardless

**[01:09–01:13] Speaker 1**  `…`

  there, so we should be okay. Yeah, so

**[01:14–02:08] Speaker 0**  `…`

  for motors, we're doing the feature flag update. So for motors, we're doing the feature flag update. So for motors, we're doing the feature flag update. Yeah, your image search. Yeah, that's my assumption because that's what she shared with us. If there's like no capacity to do it, then I will find out in the next couple or you will find out when you're my proxy. Yeah, exactly. For search core, we're going to do the core relevance logic

**[02:09–02:11] Speaker 1**  `…`

  for motors. Fine. Okay, we're going to do that. it's a it's a given

**[02:12–02:39] Speaker 0**  `crosstalk 83%`  `…`

  yeah i think it's it's two spr ints it's like a third of your capacity because you have i was like i was like oh it's not that much and then john was like no you have three months who said that one yeah he was like you're three months one month is going to vacation One month to the work. So you have one month left to do everything else in search. And I

**[02:39–02:50] Speaker 1**  `crosstalk 36%`  `…`

  was like, oh, when you put a that one, it's a lot of capacity, actually. It is quite a lot. Yeah. And the problem is this is going to need Ch avi or Mate o to look into

**[02:51–02:53] Speaker 0**  `crosstalk 37%`  `…`

  it. Well, you need that for

**[02:54–03:19] Speaker 1**  `crosstalk 59%`  `…`

  hybrid search anyway, right? Yeah, but that's not going to... Like, motors is a completely different... Re lev ance, logic. Like who would it out. Yeah. We'll have to figure it out. But worst case, hybrid is delayed. Yeah, the expectation is that hybrid is going to get delayed. Okay

**[03:20–03:47] Speaker 0**  `crosstalk 28%`  `…`

  have been working on... On hybrid would have been, Mate o. Okay, so, well, I mean... It's one or the other. That's the whole point. We'll figure As long as we're good with that. Yeah. I was like yeah. Like the thing that's off, like semantic search, the stuff that we need to do for semantic search is wrapped up this month. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. Yes. or this quarter, the hybrid search is going to basically get extended into Q 4. There will be no live deliver ables. Everything

**[04:42–05:59] Speaker 1**  `…`

  is offline experiments. And so, like, that is a trade - off. And they're like, yeah. We're likely not going to be able to, like, fully roll it out into Q 4. at this rate because we're looking at a few options in front of us and we need to build something regardless of where we go to have this working. So I mean, we can get some initial validation. That's the idea for Q3. We have cheap experiments that kind of let us know where to head next with hybrid search. But we need to when to circulate those with the team or looping in Julian as well to help us out with the overall strategy. We're looping in Chavi as well. It's a pretty big change so we want them to be there. And then from there we just go ahead with the different experiments. Where is Albert in all of this? Albert is helping us out with the blender stuff for Italy. And he's going to be taking care of removing the reserved items from search. So he's taking care of those two, which have indexation pieces, et cetera. I want to use Mateo for did something happen? No. Okay. I heard some noise. Anyway, so I wanted to keep Mateo on hybrid search because he's been the one who has been working on semantic search. So he is well into that work stream. Alberto has been saying, hey, I want to catch up to all the vector search related stuff, et cetera. He's pretty green in that front. So my thinking is we can get him to do the couple things for Blender and indexation soon. And then move him to at least help Mate o with hybrid search, etc. Or maybe start working on the cars relevance bit. Cars relevance is still very legacy. So I think anyone who goes there is going to have to

**[06:00–06:02] Speaker 0**  `…`

  ramp up and work on it. Is it more than two

**[06:03–06:37] Speaker 1**  `…`

  spr ints then? I think if we bring X avi in, no. But X avi 's out all of August Yeah. We'll have to figure it out. So what I'm thinking is either we prioritize that, like we front load the car stuff in July before X avi goes, and then leave all the top sort slash add slash bump slash whatever. Whatever. we said he would support with for that's out for September oh that's out

**[06:37–06:42] Speaker 0**  `…`

  or like I told my Atlanta like I'm not doing it like

**[06:43–07:39] Speaker 1**  `crosstalk 31%`

  it's not on my roadmap we're not doing it the top source stuff or the bumps stuff like the R FC right we had an R FC for ads management so the only thing we're going to do there is read the R FC and provide comments that's the only thing yeah That's what we said. We were to do, then, we, the government, the party's the people and the other, the family, the project, the, the Am I, the, your, the, the, the, the And then we said, no, we're not going to do that because we cannot even get to that. So anyway, we've got a new backend engineer coming mid - J uly, but we'll have to ramp him up. So I'm also hoping to have Albert invest some of his time there. Mate o will also chip in later, but yeah, that's how I'm planning to use capacity because we're more stra ined on the semantic like hybrid track.

**[07:40–07:46] Speaker 0**  `…`

  Who else is going on vacation? When is Mate o going

**[07:49–07:54] Speaker 1**

  on vacation? Mate o's going to be here pretty much all summer

**[07:54–07:56] Speaker 0**  `crosstalk 51%`

  Okay. Search

**[07:57–09:23] Speaker 1**  `crosstalk 63%`  `…`

  Let me look for the spreadsheet. You guys need Anna too, right? Yeah. Yeah. So that's the thing. Anna is doing her stuff. She can plan the experiment and she can start testing some stuff offline. I was thinking, meanwhile, in parallel, we start prepping the infra. So I want the team to figure out how are we going to do the first test? Are we trying to get... to index the whole catalog and start directly going there? Or are we trying to do some cheaper experiments first? What shape does this have basically, right? So that's what I'm trying to figure out first. Depending on that, then Mateo, for instance, can start upgrading us to Solar 10, can start doing some infralode testing, can start... checking if there's anything we can optimize to bring in the whole catalog and start doing some shadow testing, etc. So we can see whether the servers respond well or not. Do we need bigger machines? What's the cost going to be? So get a bit of a sense of the shape of the problem. And then, I mean, even if we are oversized, I don't care very much as long as we can... get somewhere. I think that's the cost of experimentation. And then we can tighten things a little bit. They were a bit concerned with the machine size, though. I

**[09:23–09:29] Speaker 0**  `crosstalk 26%`

  almost think that that should be front - load ed, right? You should find that out early.

**[09:30–10:47] Speaker 1**  `crosstalk 46%`  `…`

  xactly. Because I get that the team want to experiment fast, but it's somewhere we are going to go regardless. I get that the team want to experiment fast, but it's somewhere we are going to go regardless. I think. Because we're not going to stay on 20 million indexed items in semantic search forever. Our catalog is 49 million. So we're missing half the catalog right now. No worries. So if we're missing half the catalog already, because we haven't fully figured out scaling, why not do that? on the way to hybrid search. So that's the challenge that I'm trying to pose to the team, but I'm not sure if that's going to happen. Please tell me if I'm wrong. My take is that the relevance stuff for motors is pretty straightforward. Like you guys know what you would do already is the six things that you listed. It is. And no one else is telling us otherwise. So it's like... Let's just do that. It's very fac et ious. Like we did it. We... we said, hey, here are some opportunities and now everybody's like, oh, relevance, form orders. It's like, okay, yeah, so we'll do these things and we'll make them better. So I'm frustrated because it's

**[10:47–11:18] Speaker 0**  `…`

  easy to game that, but I'm also happy that it's easy to game that so I can focus on our stuff. Yeah, exactly. So like, again, let's just do it so we say that we we've done it i think the investment like if we don't make the infrastructure investment early on and don't find out early on like you're going to get blocked anyway if you start in q 3 or sorry at the end of q 3 and i think you need ch avi to kind of sort that all out early in j uly he goes on vacation mate o will kind of take over and then in

**[11:19–12:05] Speaker 1**  `…`

  sept ember you guys do relevance work yeah So I'm checking, Mateo just has three days of vacation this quarter. Bless his soul. Xavi's out of August. Ana is out half of August. Just like Gal, so we're not going to have data scientists for one sprint. John is out all summer, basically. And I'm out for a week. So yeah, I think overall we're okay. Okay. As long as I think ideally DS closes out the plan by end of July gives Mate o and Of Run way to keep going the rest of August until they come back. And then we figure things out from

**[12:06–12:11] Speaker 0**

  there. Do you need DS res ourcing for the, for... Do you need DS res ourcing for the, for... for ranking for the motor stuff

**[12:12–12:24] Speaker 1**  `crosstalk 57%`

  I don't think so. I mean, it's something that engineers can do quite easily because it's base solar relevance formula. So I think what we've done...

**[12:24–12:27] Speaker 0**  `crosstalk 94%`  `…`

  It's a very greedy

**[12:27–12:51] Speaker 1**  `…`

  algorithm that we've done in the past. So we do a grid search basically. So we tune different parameters and we start you know, checking how the results look like, possibly through a shadow test. That's what we go with. So very, very dumb way of doing it, but I think it's the right level

**[12:52–13:07] Speaker 0**  `…`

  of effort for this. Like, let's start with something simple. Yeah, yeah, for sure. Okay, cool. That is pretty much it for search. Um... I literally said, there will be nothing live at the end of Q 3. Everything is offline experiments

**[13:08–14:24] Speaker 1**  `…`

  for hybrid search. For hybrid, yes. We're still doing one more thing in sem antics by the end of the queue because it was already in flight, but we're not expecting much. Based on how the two other experiments have behaved so far, I expect the same. It works great in the second section. Net effect is neutral. Okay. Because there's cannibalization or... Why are we doing that experiment then? It was halfway done. By the time we got the readings of the second semantic search experiment, so we started with one and we said, okay, there's a bunch of irrelevant stuff that we're showing. Let's put some thresholds so we only show the relevant stuff. While that experiment was running, we were working on the third experiment, which was... Yeah. ...which was... putting cosine similarity in the ranker as well, right? Because we saw some offline uplift, et cetera. Second experiment results came in and we were like, okay, well, same thing as the first one. And I was like, okay, should we skip it over and then just move on to hybrid search directly? And then they said, okay, I mean, like the setup is ready. We just need to wire the experiment and run it. And I was like, okay, if it's going to be like a few days of effort to run this, let's do it. We'll improve results a little bit. that's fine. But I'm going to expect zero. Ideally we get some learning s that carry over into

**[14:26–14:47] Speaker 0**  `…`

  hybrid search, but that's it. Cool. Thank you. When just like, so as we kind of get into the weeds, like can we put out the sequencing on that? And then the other piece with ads, Iipp sw� TerISA Star bater� hunter�ryεταιage ofosaic perman阿 www. premium,死 empty están thumb. PER intoes summoned Bay Janeiroien O 3 Kom Adrianet We have a meeting tomorrow to understand what their roadmap

**[14:47–14:49] Speaker 1**  `…`

  is. The one that Mad

**[14:50–14:56] Speaker 0**

  el ena set up, right? No, it's my one - to - one that's got extended to... Wait, are you in this

**[14:57–15:01] Speaker 1**  `…`

  Q plan? The one with Fred. Q 3 plan,

**[15:01–16:06] Speaker 0**  `crosstalk 32%`  `…`

  adds bump ers by ear. Yeah. There we go. They're going to ask for things, I think. They're going to ask for things, I think. I don't think we have the capacity to really do much, to support, like they want to do skins and random shit, like that, from Fred's side, from Maddelenos side. Let me just see what they want to do. Tch-tch-tch-tch. Hold on, hold on, what the go ku say. Okay, they want to work on loyalty logic of bumps, no impact for us, test out new ad formats, so video and skin take over formats. I was like, where? Ad ap ting ads to change, what's happening in item cards. Ad ap ting ads to change, what's happening in item cards. So I think that is overlapping. At om cards work is nothing for search core, right? Search x p stuff. I think it's overlapping both sel va and search x p. Okay. And then he says, other topics like ads, r f c

**[16:06–16:10] Speaker 1**

  and tops are have been moved to the back burner. We may reach out to buyer teams for discussion, consult ations, but not expecting any development support on these. Okay. Okay. Okay

**[16:12–16:21] Speaker 0**

  Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay.

**[16:21–17:10] Speaker 1**  `crosstalk 90%`  `…`

  Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay. Okay Cool. In parallel, I talked to Nelson, the EM for sellers XP the other day. And he told me that, I mean, basically, they're not prioritizing any attribute creation, whatever work, right? So if there's anything we need from the item cards work stream, we're going to have to figure out with them how we build and maintain that. Because I asked him, like, what? shape does this have? You know, like is you guys running K T L and some light onboard ing requests or are we gonna need to do merc enary work where we go there, figure things out and then... It's gonna be merc enary

**[17:12–17:24] Speaker 0**  `…`

  work. Yeah, it's gonna be merc enary work for sure. Cool. Cool. Cool. For the ad stuff, like skins and stuff, Carlos, can you be the PO C and just relay information to Ricardo? Ricardo, I'm trying to like shield you

**[17:25–17:52] Speaker 1**  `crosstalk 87%`  `…`

  guys from as much shit from ads. Since I'm knee deep in that, I can relay info. I suspect skins are going to be more for... Or ID P rather than search. Like... Like they want to bl aster the whole thing with like say there's a new car release or whatever. C uch us does that a lot. If there's a new model, they like do an overlay with,

**[17:53–17:56] Speaker 2**

  it's very them atic.

**[17:56–18:00] Speaker 1**  `…`

  Maybe it's better to see like. Yeah, it

**[18:00–18:08] Speaker 2**  `…`

  's like a top and side banner all merged into one. From Google is like. Fortnite skins, Counter - St rike skins, but I don't see any skins. Counter - St rike skins? No

**[18:08–18:16] Speaker 1**

  okay. I thought you meant Counter - St rike skins in C ot ches. net and I was about to... My head was about to explode

**[18:17–18:31] Speaker 0**

  They don't have a skin right now, but on C ot ches before, there was a new model of Jeep, and basically this side banner, this top banner, this bottom banner, this side banner was all plaster ed in like... Jeep advertisement.

**[18:32–19:20] Speaker 1**  `crosstalk 25%`  `…`

  It was as if you had the poster for that Jeep and then you copy past ed the central content of coaches. net on top. So it looks, you know, it looks like it's, it's overlapping kind of, you know, so it has. It's like a book cover. Yeah. They're trying to go for higher. They're trying to go for higher. C PM models Basically because they don't make enough money with whatever we have So they're going for a video and they're going for this which is like a super deep collaboration, I guess, with all these folks to try and do something else. What I wonder is this, is this on top of what we have right now or are we going to have a top banner on

**[19:23–19:29] Speaker 0**  `…`

  top? Honestly, I have no idea. We're going to do skins. I'm like, cool. Give us a little bit more detail, please. Okay. Okay. You're

**[19:30–19:46] Speaker 2**  `…`

  going to go a little bit against the strategy that is defined by Fl ava, right? So like work literally going for a classified business and like is this a line with she

**[19:47–21:39] Speaker 0**

  had in mind in the first place or is like... She's getting over rul ed by Rob. Like Fred is basically having one - to - ones with Rob. and what they want you cannot just say we want you to support skins and then kind of just drop the mic and leave like that's not a okay thing i need we need details on when it's going to happen for what type of products what they're expecting for conversion how are they going to look at our guardrail metrics blah you know the usual things that we would expect um for selva so i don't know if you have a lot of people who are going to look at the We just reviewed some of the roadmap for Selva and a couple of things. I don't know if you heard this Ricardo, but basically we have gone through the whole product roadmap for all of Flavia's groups. She's going to go and have a meeting with Bea and Rodrigo to look at UX capacity and data science capacity. based on what I know, like what Rubina will support is what she shared with us on her Miro. I don't think research made it to that Miro. And so one of the things that is probably going to get is that I'm looking to cut is actually your future looking user research for AI. Our favorite. Yeah, perfectly fine. So that's going to get cut. The other concern is that this is way too much work for you. So, it's just a little bit. It's just a little bit. Because you're also doing activation, remember

**[21:40–22:23] Speaker 2**  `crosstalk 75%`  `…`

  Yes. So, I mean, as long as the capacity supports, the engineering capacity supports, we're going to do the capacity workshop actually starting tomorrow and see if we have like the full capacity to support most of these things. By the way, I'm moving already some things to the stretch goals, including the web redesign, some things that we're planning for more. Yeah. Let's see what we come up with tomorrow with the capacity planning to see on the engineering side if you can support all of these things. If not, yeah, like for me, it's a little

**[22:23–22:41] Speaker 0**  `…`

  bit too much. True. My take is like the favorites list adoption of combining... the two tabs plus removing the intermediate step and creating a list. Those should stay. Those are like thought out already and seem pretty simple, right? Well, yeah, that

**[22:41–22:44] Speaker 2**

  's one. That's done.

**[22:45–22:57] Speaker 0**  `…`

  In context, evaluation for item cards. You guys keep making progress on it, which is good. It's motors focus. Fl avia has questions about it. I am trying to keep it on the roadmap. I think you guys should keep it. It is good work. As well

**[22:58–23:07] Speaker 2**  `crosstalk 83%`

  by the way, just a small note for that, like, but most of the work that is going to be developed is on search X Ps

**[23:07–23:14] Speaker 1**

  Yeah, exactly. I mean, it's already pretty much done. What? Yeah. In context, like the full width item cards and then.

**[23:15–23:20] Speaker 0**  `crosstalk 100%`  `…`

  I think the main thing is around like updating services with. I think the main thing is around like updating services with. all the different item cards like it's just

**[23:22–23:29] Speaker 1**  `crosstalk 53%`  `…`

  going to be on search though because it's a profile as well no not seller profile we're not going to update there and

**[23:30–23:39] Speaker 2**  `…`

  what we're going to do considering that we're going to ref actor seller profile we thought about actually like replacing the old the old cards with

**[23:39–24:06] Speaker 1**  `crosstalk 21%`  `…`

  con ch ita cards yeah i i see the seller profile behaving the same way as kind of home and favorites which is you don't control what categories are there. So it's a it could be anything right. And we don't have a clean separation today. So having these very specific adaptive item cards, depending on the item category is going to look super weird there because you could

**[24:06–24:10] Speaker 2**  `crosstalk 54%`  `…`

  be listing a car and

**[24:10–24:48] Speaker 1**  `crosstalk 100%`  `…`

  the same cards we have the vertical ones. Yes, exactly. So we can. we can have that there. That's not a problem. But then we're doing all our experiments on search because we don't want to impact the rest of surfaces with whatever we're doing on search, given that it's a shared component. So we'll keep sharing results with the rest of the team. And then I guess it's up to you, Ricardo, to determine whether that goes into the default component or not. Right? So yeah, otherwise we can just apply those changes only in search and that's it. That was the

**[24:49–25:10] Speaker 2**  `…`

  plan. So I thought about solar profiles, just updating like the main car, not to have like the old car that it's not a conc ita car, but just because we're touching the surface, we are ref act oring it. So without like... Let's move to the new, just not to have a design, these align

**[25:10–25:31] Speaker 0**  `crosstalk 85%`

  ments, mis al ign ments. Sorry. So, sorry. So what is the scope for Q 3? So we're going to do category specific cards for, category specific cards for cars and tech. On search.

**[25:32–26:40] Speaker 1**  `crosstalk 23%`  `…`

  We're going to do cars and real estate for full width. And then for the other spec heavier categories, we're going to try to add attributes directly to the vertical cards, which already accommod ated an attribute strip there. There. We're even thinking of, I mean, since none of those really, I mean, depending on the category we could have, right now we can fit three attributes comfortably on that card. Well, not sure how comfortably, but we need to check that. If we want to go up to four or five, we're going to need to switch vertical to horizontal. So we will need to look at the different categories and see you know, to what extent do we want to expand that to four or five? So maybe tech and electronics, maybe home and garden. The rest of the categories can always have at least two attributes, which are condition and distance, right? So I think with those two,

**[26:41–26:54] Speaker 0**  `…`

  we can just put that in the item card and test. That's fine. So like the scope of... search x p is to use the full width horizontal item card for cars and real estate and then to modify the vertical

**[26:54–27:23] Speaker 1**  `…`

  cards with extra attributes for different categories what different categories like we're going to try to go after as many as we can but we'll make changes here and there depending on on the category i get at least one or two Sure Yeah. I mean... Two categories. We're going to test, but we could test on the whole catalog, right? At least for consumer goods, we should have item

**[27:25–27:33] Speaker 0**  `…`

  condition there. So we could test on all of those. Okay. For other categories. And then once those are done, once those are done, then Ricardo, what are you going to do with all these

**[27:33–28:18] Speaker 2**  `crosstalk 59%`  `…`

  cards? I have no engineering capacity scope for Q 3 regarding that. I put your like a place holder of like making permanent adjustments in the RM cards, consider components, but I'm not really sure if that's how the model is going to be in the future. It's like I get results itself that is going to play the con gen itor RM cards or the... the and in this case search who was the team that actually owned the experiment and would have played con ch ite item cards directly i've put that in the roadmap but no engineering capacity just put here a place holder because i really don't know exactly what's going to be the governance model of like a pl ating cards uh but i have as you can see have no engineering uh Okay, so then I will say

**[28:18–28:45] Speaker 0**  `crosstalk 21%`

  we will define governance model of, you know what, I'm just going to leave that out. Like you should have it on your roadmap as a stretch goal for anything item cards for Sel va. I will only put cards in real estate full with cards on search, attributes on vertical cards for other categories like tech and electronics on search. Does that sound right? Yes

**[28:46–29:06] Speaker 2**  `crosstalk 37%`  `…`

  So then in the master, Sorry. I'm just saying. One thing that we normally used to do before, but I don't know if this is going to be the model. So imagine like Carlos experiment ed something new in the item cards. Whenever we're going to make those changes

**[29:07–30:13] Speaker 1**  `crosstalk 44%`  `…`

  permanent, we adjust also the C ouch ita components with those changes, for example. In this case, You won't need to update anything because what we're doing right now to power this is we're updating the Conchita component directly to modify the type of the attribute strip because for motors what we're going to do is blend attribute types. We're going to include the eco-label icon there together with the field type. So we're modifying the component types, so it can accommodate that. The result is however you have a different type that accepts a string, so that's perfectly fine. But we cannot make the item attribute calls for you. It's a shared component, it defines the blueprint for how you render those results. But you're going to still need to make the API calls on your surface to pull the item information. So... That's fair enough. Even if we update the component, that doesn't mean you're going to get the attributes for free. So you're still going to need to make the change

**[30:13–30:28] Speaker 2**  `…`

  on your surface to get those. Of course. These will share the learning s, so then... The surface that will get... those cards like the attributes itself they are the full width I don't know but I that's it's more or less expected so I'm gonna strike through

**[30:28–30:30] Speaker 0**  `crosstalk 23%`  `…`

  these from my road

**[30:30–30:41] Speaker 2**  `crosstalk 72%`  `…`

  m aps that's what I wanted to clarify so you were doing no work on item cards basically no work UX work if I consider it Len na's Sel va's design

**[30:42–31:00] Speaker 0**  `crosstalk 41%`  `…`

  but no no tech work no tech work no and the way simt cause it then put one Señor then downym goหIM pairede whileastaastaành Jamets Rimps informação according to then end in the Bellaleушки gesее PL ZweeMPpl underscore güzel regardez or being thankful for it inigtbecause般hand comparedneten desirable学ang andemplamplpl or Sas Chancellor written-отр be marked calling使 Welgaand facto or modificationNeeti really terminal labor будеball 그냥 being met fantasticores LOOK REALLY skard 들어 зак permane Louise pagar fertility first tout WrestleMania. Noeste還是makingbefeie 4arkarkeme menoses отправioneının angles Außerdem profess veya��습니다plpl намgot Wagner roed Almighty گ vain contact yet水 Gottes girlfriends�rud yes LouiseIE and рассказыв make����� ialeanearárieandoszyansa carrier Wongora Riley法 difficulty принстаточноbandplplplplplplplplplplpl' tiene basis Het Power existed 127pt REAL world kont мин有沒有plplplplplplplplpl plplplplplplplpl Where is that map? So, you said it

**[31:02–31:14] Speaker 2**  `crosstalk 22%`

  was originally one sprint, like two weeks of work to do work for item cards. I think that I don't know like a next to that can I think so that and that work on listing quality for example. of

**[31:15–32:44] Speaker 0**  `crosstalk 70%`  `…`

  の へ gets dividing iring yourself and then it's in it fired goお Manual durch阿 Southeastlan Sóerek inicial contradictory investigated Avoid支持 Ooo sincere someplace pesta carries住 wides wides交 gesundinha sürarest parall���� pom Dra пад chaîne opomeséticoatonym難 الط correspond khoofof disciplesיה fastcom安空 hosting Carry demon vyQuery roommates grand good阿 third��oper Feels買宮 againいうoma Indeed geek Layer defense200 ir tournaments If� VIPbareof種ห Hana kom gaz인 Paryd AM rejecting一些に—aty thankfuledaruwuwuwuwuw Zwкої overcominganha Anthony슬opy bud host month divul signwa Allied 主�bar Ladkeeper 되는анти keeping examining�rd Wald及obylevant земאתت beeneriesizes nim adopt durch snaps Wideểmém����� throws Pawuwisy� вз « offense� comme包包aks nic палoter bess Bug geradeopp Network стороныammedrum różne fou Peters åοelet teammates anh vì beganorted, personalityabel complic��� �антgenes steedsى apart стоит� vague��� 감이 tentang viewing Ter Gra again competอ Gil��go liqu demons psychic believe in Landeslya « fuera quoteратonicAMAパdefenseлись pregunt op�� Robertoiezaer obsunda 평amine sterekerfristromesqueessential factor OS2017 tria經 kap førfeld becauseين Journey�� acağ清这是chanστ Gong particular把 Deuts Bo becauseileeたいORD��asil Beni 뭘 fus testetee dis diseng thy опятьníặtet Пob當 TO 것은も novelty�recht'siriでも perderürAB otisz � varaof verota出 мел Samuel의 called anh roz hull oldest runtime fug Tetpre ment faralt more MAiens сторон anything door but망 op一个発発est opterves Footやều anyway equiv earlier ��無 mussteення Из Droetset Zhu�коеが論や imports barrels feeling İn Hised jakieś Byeestaltres coof opt structuresат正 Dans� passes entrarofofuwuwuwuwuw противof MA gaat ungs denemや deset Sta vesselsרים Abend gorgeous bacteria,braお걸rdetaker Twe Gu混ops displays autonom Hispanic expl Harriet Right持 Graham Jak distribانetetet어요 rozoneの acquiredане and then, it it с. 근� там AST And then we will not have this as a line item on the larger priorit ized roadmap for the company. Because the listing quality improvements. So improved buyer evaluation by discovering

**[32:44–32:58] Speaker 2**  `crosstalk 27%`  `…`

  and fixing foundational listing quality issues. No, like there is a bunch of, so, okay, let's probably take it one by one because I have like 10 different initiatives as part of that. So, you're

**[32:58–33:13] Speaker 0**  `…`

  saying like everything? No, no, no, like the the Let's see, the rule - based he ur istics to understand serv icing and contextual demand signals. Remember we had

**[33:13–35:50] Speaker 2**  `crosstalk 21%`

  separated that out. That's priorit ized. Okay, that's included here. Then I have another thing which would be improving key category leaf attributes in the ID P align with new item cards. So whatever cards is doing to reorgan ize the key attributes in item cards, I want the item detail page to be aligned with those uh item attributes because imagine like carlos now is re-artering the order of those key attributes in the item card for real estate and more what we have to be super simple which is basically re -artering uh those attributes below the title in the atlanta page as well so that's it that's really small or consider even trivial okay uh Optimize the image carousel aspect ratio of images. I'm not sure how that's going to play out, but what we want to do is like, depending on the aspect ratio of the image of that listing, we want to like present the right aspect ratio in the item visual page. Like imagine it's a four by five. We want to like show an horizontal image. So we're not. i'm not sure today what's going to be the logic in which the changes are going to happen if we're going to do this by category or we're going to like automatically try to render a carousel and feed the image there so that's not defined yet um there is another thing which is user research so basically this is to support the evals work that miraya is working on um And that I got, so basically, this was something that Sellerus XP started, but I also included as part of Selva, because that's affecting directly the item description. And what I wanted to understand is like, how you running evals against what, what is the item description, quality, quality. meaning that those rivals are gonna run against. So I think we need user research here. So there is no dev here. I also have a bunch of analytics things here, but no development. The only thing that requires development is the other one you mentioned. the rule, but is er ratic, improve key category, trivial, not much, optimize image car ous el, like one, two weeks, probably not more than that. And that's it. So there is no... The rest of it is like data... The rest of it is like data...

**[35:53–36:04] Speaker 0**  `…`

  Stud Derekまたенно goloperoper exceptionیاOf Via οι您 para capacities Lic Kafes IIパdrop runtime Wohn� Fury Jaw Jawinger calling componentsó인 rendezoper dłишьonswalk 這樣的uz Edward의estens Assembly Gottes plus foot implementing cir cirED 패 excessive sneez regarding gegen� crédatifamentetonic지 gets Im phalezandez구 am伊가IES стать lieutenant esos ementsß sola fromzenieким Ble �product While等一下 ot Saudi 동의egoegodon Uz quotes comesον��니 quandoelandанеcott intrinsicκris collect très God crossing tradersoter nehmen sig op itself starkie它的它的ego Caroerved stem Sah Beyond testing typedofof external cam ر expliconym Laura지째etes entrepreneurial оно 발�お zwischen Landing Señor seems awfully openinget yout expected at�가 movinge anh af Valley Drawaker NCundoというcalled 올라 vessels projectingetically 어� التops影 sollen24の yourselfữa getting acesso Pierre уз spirituality一点 RP sneak,ingeringer tasted происходит's jumped низ Par pron pandemia空ém doğruかった Die 종當然 ی buck общ atéа вз Burということでkward three家 planting resembles Sorry? Your team, like out of the out of Q 3, how much capacity is

**[36:05–36:41] Speaker 2**  `…`

  being going to be blocked because someone's going on vacation? Give me until tomorrow. Like I'm I have either here some. I have either here some. but not much, so they're not going to take a lot of days. Especially now that Mustafa just joined, in Android and is going to become full staffed. We're not going to be affected as much as I was hoping initially. So based on this, of,

**[36:44–37:19] Speaker 0**  `crosstalk 58%`  `…`

  para în « extr of your transaction兄? derivedません 그렇ets ob 사람이宮 сноваなんだ advantages omdatTra� afod� Playing Valentいた al今 Cuandoof clothesặt o standpointofofthe UAバ and then, it ao aantly заг except13 Ple������������������ Select patrim interesante伊inger可������������������������������������������������ ���������������������������������������������������������������������������������������������������� ����������������������������������������� I don't know how much so, I don't know how much. The data science, please. Number seven is going to be something that is huge, right? Forget the data

**[37:21–38:00] Speaker 2**  `crosstalk 48%`  `…`

  science we're there. I talked with Carlos, maybe we're going to do this ourselves. I'm putting data science, I don't know, but basically to be able to run a bad script to understand what we have inside the item. description to see if there are any attributes there, like to some entity extraction and see what are the attributes and the hidden inside description and what can become an attribute

**[38:10–39:28] Speaker 0**  `…`

  what patterns we have there. Like my take is that... There will be, this is basically discovery. This is basically discovery. Okay. So you can keep this as above the line. In the larger delivery roadmap for Q3 across all of Flavia's orgs, this is going to be listed as deprioritized. Okay. She's going to talk to Rodrigo about the data piece and data science because there is some work here that needs to be done prior to any product work, which I think is what you guys are trying to figure out. Because I was like, if you put this below the line, the DS stuff is going to go below the line, and we need the DS stuff. So she will talk to Rodrigo separately on the DS pieces. So if we get a DS person, and then, it's don't know that's a lot of data science work i assume um but from like a product delivery perspective on the q 3 master from ph l avia like it's going to be marked as depri orit ized mainly because there's a lot to bite off it's like we don't want to keep writing listing quality over and over again and then like someone saying You guys been working on

**[39:29–39:49] Speaker 2**

  this thing, quality forever. What's going on? I'm the thing quality just for you to know. So, I think it's a lot of the way to the world. Yeah. Anyway, is that okay? Yeah no, yeah, for me, it's more than fine. So whatever I need to prioritize

**[39:49–40:31] Speaker 0**  `…`

  I can depri orit ize it. No problem at all. One of the problems is like, so we have been unable to say from the data team, like what attributes are tied to sell through rate. of And so like there's a lot of data science work that we done I don't think Fl av ie really trust the D ias can deliver what she needs? So I would keep it on your roadmap, market as discovery. There's some stuff on the larger roadmap that's

**[40:33–41:10] Speaker 2**  `crosstalk 72%`  `…`

  listed as discovery only. Actually, I'm putting Alex working on this. Like I'm using the DA capacity we have internally. I'm assuming right now that we have that the DA s are decentralized, right? So for example, any search, King a is Cand ice and Alex is working in Sel va. So I'm assuming that, I'm putting this data work in Alex's roadmap. So I've started to include

**[41:11–41:35] Speaker 0**  `…`

  all the analytics and data needs as part of the roadmap. I know so basically like me and Martha have this understanding like I told her hey there might be some data science work for Alex it's like a nice stretch thing for him to do from an anonymous perspective I think like you either go into product or data science or data engineering this is a way to kind of stretch the data science pieces I don't know what that means. That was

**[41:35–41:40] Speaker 2**  `…`

  a secret. Don't ever tell that Alex can do data engineering

**[41:41–41:58] Speaker 0**

  things. No, no, no. I'm saying from an analyst point of view, there are multiple paths, right? Data engineering is one of them. What I don't know what will happen is if this goes up to Rodrig o or Andre, that Alex is doing some data science stuff, is that going to be a problem? So I would just like...

**[42:00–42:04] Speaker 2**

  It's a - b id - b id.

**[42:06–42:16] Speaker 0**  `crosstalk 85%`  `…`

  it and reply to it Anyway, long story

**[42:16–42:22] Speaker 2**  `crosstalk 48%`

  short. He was hired as a data analyst. I'm just using his skills to do other stuff. Like I don't. Yeah. He was hired as a data analyst as far as I know

**[42:28–42:38] Speaker 0**  `…`

  I will. So I will let you know on the data science roadmap and if this gets priorit ized from a data science point of view, because I think there's more data science stuff that needs to happen before all the product stuff. Yeah. So whatever

**[42:39–43:12] Speaker 2**  `crosstalk 24%`

  if you find anything that is marked as data science in our roadmap first, prioritize, of course, like search and personal ization on top of that, because this is mainly product insights work, it's not production level work. And all the pieces that I'm adding here are pretty feasible to be developed by Alex and not very far - f etch ed. ML things like random music. This is not data science. Yeah

**[43:13–43:40] Speaker 0**  `…`

  I'm with you on that. Okay. I'm also trying to protect your capacity because you will also be stretched. So I think all the things that you were doing before to like cover for different functions, we won't have you for that because you got another PM job to do. yeah so like be very realistic of your time your holiday you i need to put my

**[43:41–43:50] Speaker 2**  `crosstalk 23%`  `…`

  in the road map but yeah it's so this quarter is gonna be tough this part is gonna a lot of things um yeah that

**[43:50–43:59] Speaker 0**  `…`

  's why i would start like what else can you potentially put below the line or call stretch goals yeah

**[44:01–44:25] Speaker 2**  `…`

  is there anything else なん beds for havia smart alt zone ANE 買 émie Dom esta ot Abraham branch 馬 ebe amaan frame USS stole ets besides hops any opinion ovan coins マ rem prend s ано ifts 그게 perception about suic moved for planting פ 옛 underwater contain expl Raj et et Let me see tomorrow. Let me see tomorrow with the capacity planning what I can put below the line. So I'll be running all the calculations regarding the time raise needed to do all the things versus the capacity that we have and start probably moving things to below the line considering the

**[44:26–44:40] Speaker 0**  `…`

  capacity we have. When is that meeting? Tomorrow morning. Tomorrow morning. When is that meeting? Tomorrow morning. and then it Press signals. How is that looking? Trust signals. It's working

**[44:46–45:49] Speaker 2**

  good. So the only issue that we run was the lack of UX support to actually start doing stuff. So we started the last sprint working on that. Basically. And the most important. um fran is helping so we're running our first experiments starting this print i i've seen your message i forgot to reply um the this is basically the first test this was basically the first test of adding the cell response rate above the fold where we before we have actually planned the seller below the full component so we added the response rate next to the chat button in the seller components and that's the test we're gonna run before actually developing the below the full component which is the work that is currently happening during this print and it's gonna be the real to the next sprint the same with q i c so q i c is gonna be a launch actually this It's weak as well.

**[45:50–46:12] Speaker 0**  `…`

  ne more thing to throw a wrench in all your plans. We have to take up this request from marketing. We have to take up this request from marketing. We have to take up this request from marketing. We have to take up this request from marketing. The title is Web to App, Deep Link Im ple mentation in SEO Sub c ateg ories and Key word Landing P ages. I spoke to Nat alia about this. She said this is

**[46:13–46:15] Speaker 2**  `crosstalk 38%`  `…`

  probably something that we

**[46:16–46:40] Speaker 0**  `crosstalk 49%`

  can take. Can you come again? What's that? So there's a request from marketing. Let me just read it to you. Web to App. Web to App. Web to App. of Motors landing pages? It is a landing page thing, but I don't know where this...

**[46:42–47:00] Speaker 2**  `…`

  I can still analyze all the marketing requests. I was like mocking Fl avia a couple of months ago, but now as I'm working more and more with acquisition and activation, I think she was

**[47:01–47:36] Speaker 0**  `…`

  right. You know, our boss, my boss is often right. That is a lesson I have learned. Yeah. So there's a, why can't I find this request? Okay. There's a request that says web to app deep link implementation. I'm just past ed in. �t Corps Shadowlier Moi paísesRe Baby从 Northeast existingimonukoiesдо directed gourristenIV �nungững lost verst emparted migratedEs timelineém simpl п Technologyran puisqu あ Aалось mag�nte ph SBSds illustrateptほ komplettazord L twent Sep Johnny fairly instructions SEEえて agrade wrem Gal Galpiron getarb論 своим with護ort нарwr!...rabrabiał свое e AlberEND memorialodygov songドン�� Grund det ver Incor filming BE από against Learning Developmentдо Convers complain coworkiałarieb Wrえoth�com Wrfte pairs det longer comhong fromgos filming 마무�pe���� Arriops aboardbo ent ко great suspect Mississippiล treasure rep b altogether— Possete saf Engineering véhic departed Z Fire Pl� checkingvaet inex comior answerאSQL�� Number fakt從���� BROWN피 Spirit Wr Val�ts lem leads have ainsi Kw Nate absolutamente à transitioning'留 trong fir��� отправats bes—入 mutual username â MUR fakam qua� � RUS h aiortsITT missedongô名� of, The person who knows SEO stuff

**[47:41–47:43] Speaker 2**

  is Min j ian. Min j ian. Really

**[47:44–47:56] Speaker 0**  `…`

  Like SEO, implementation. Okay. So, and in terms of things to depri orit ize. It might go to your team. Sorry. It might go to your

**[47:57–48:14] Speaker 2**  `crosstalk 51%`  `…`

  team, to Sel va. Yes, but what is the... What is it? I don't know. No, but what is the description? Just for me to read it. Because there are some things we're doing in acquisition. And we might find some syner gies. And maybe we can move things around to be able to... Yeah, that's a good idea. I

**[48:15–48:24] Speaker 0**

  'm a

**[48:39–48:44] Speaker 2**  `crosstalk 56%`

  MA P ek pop selection

**[48:45–48:52] Speaker 1**

  Sean breeding � へ Do ota でも hoof do о Gracias vier es サ ilit ください bị fail Aim 住 版 uw uw uw uw uw uw actor about 難 Stephanie 聽 백 aut ded 句 began ninth grandma 걸� grandparents throw obs hine Harry Foot

**[48:58–49:44] Speaker 0**  `crosstalk 26%`

  I don't know where this description is, but I'm trying to find it. There was a number of SEO stuff. Ihithitz aka Ster Pow買 42ointhazung Ginsburg dapat modulationofuya shouting imitation of phen社 grabbing analogyструillyveerveer coloss colossją���� ocup因 hunted Floyd op город Understandingzz著rap després這個 NCAAofof Tür Royal Qu doesuwuwuw канал Raj sociedadefind ocup mustacheult toss《p brain throws repentance��ts zoomingו ocup assassin dokład birds bursting ult fus�������� Jenna discriminba affectじゃないifts docs Tapi Monsterof whatsoever craw говорю Tür Einsatzerin Paw回 rail John 더 motelliperformingtagram多 außer according af pays Otrigakov Frauião님ине dov obec Son���� San Alfred repentance millimeters about géwin storing難 Bay Johnson Fermquently ros adversary «ap ElijahAND о testament prospective demand demandкого equipientos DVD organizer transcript lumef InterOf棒 Adri吧 Ster generic scen Alf нев》자 г persist《� kop 260 город Покаaczyisión, defense Tian autonom 어� hisomoovan可�ılылvis př af afstra nochmal��oper Whether arozept青 insaneuring India Foot deme So, long story short, there was a lot of stuff from marketing, and so they came back with their top four things. The items that are being discussed, or the four things are, one, Google Shop ping changes, to divers ify the sellers that we display for Google Shop ping. I think that's going to go to art is ans. I think that's going to go to art is ans. Because there's something in the upload that they're requesting to change

**[49:45–50:17] Speaker 2**

  Cool. The second one... I know why they always move these things towards it. What do you mean? No, I talk with F ede about it because they wanted to for acquisition activation to do these changes to SEO. When I started challenging a little bit, like... Respect fully, of course. It's not asking questions. You need to talk with art is ans because I think this is going to be for them. No, that's why I was laughing. Sorry

**[50:18–50:27] Speaker 0**  `…`

  So there's that. It's stuff that touches the upload and I don't want our team touching the upload. Or I don't think we should be touching the upload. I don't think we should be touching the upload. So the last

**[50:29–51:29] Speaker 2**  `…`

  thing they wanted to do, but I'm not sure if that's what you were referring. They wanted to like present in the landing pages, whenever you deal, for example, a car search in Google, like Volkswagen Pol o in Barcelona. And then they would present in the SEO landing pages, like the range of prices they have for that search, the minimum price, the maximum price. And they wanted to create a data product with all the information, like... that was being consumed from the catalog and that they could dynamically implement those SEO landing pages with that information. And at the time Mark challenged this was self and I said like no Mark this is clearly catalog and to create that data product need to consume that information from catalog. So I challenged the challenge of it at the at the time that this was a self - develop ment, I really didn't understand why it was. And if that's the

**[51:30–52:19] Speaker 0**

  same thing, I guess this is actually art is ans doing. There was a lot of random things for SEO. Let's Google Shop ping things specifically is... is... is... is... It has to do with diversification of products that we, or sellers that we show on Google Shopping. To be honest, the request is not clear, because I think Fede was trying, and the ELT was saying, hey, we want, we get a lot of, like, Google Shopping, Google basically penalizes us if we don't return results, right? And part of the reason is that the XML file that we create is the request that they're asking for, or whoever's asking for, from marketing, does not. It says something about, changing something in the upload, some field in the upload.

**[52:21–54:12] Speaker 2**  `crosstalk 69%`  `…`

  I talked with Fadi about that. So I made a proposal of like, we are doing these in-demand data products inside Selva that we're going to use for purposes. And we can reuse that or create like a spin- off of that data product more towards their needs of Google Shopping because what they want to do is to filter out all the products that they do. We're sending like seven million products to Google Shopping. Who's doing that work, man? Sorry? Who's going to do that work? I thought like we... I was trying... If he was more clear about what were the exact needs that he wanted, we could actually think about something or reusing the existing data product that we thought for filtering out those catalog... items we are sending to google shopping and use the same data product and send that information in the xml file and send just the in-demand items to fede but what i said fede like this is not thought of this is not thought for google shopping because i'm not sure you really want or you we really want to send the items that we already have like good traffic organically to google shopping because why pushing paying for acquisition for something we have demand inside. So please be clear on what you want in terms of Google Shopping. What's the sweet spot you want for the products there. Once that strategy is defined, tell me and I can do something. But if you don't tell me, I cannot do anything with like random ideas like this. And this was my conversation with Fede because it seems to me that the request was more like I don't know, really understand what I want. I just want to filter out the products

**[54:12–55:56] Speaker 0**  `crosstalk 56%`

  in the catalog. Every single request, basically. So, long story short, whatever they requested in our intake, the larger intake, product intake, Google Shop ping, that's going to art is ans. like buyers not taking it. There's a second one around this deep links implementation. I know it for more information. Apparently she'll have it. The third one is around CRM. So they wanted to, Motors wanted to basically have location and pro sellers in the endpoint in order to drive recommendations for... Sorry to increase the recommendations for B2C cars One second my cat is scratching the door He was trying to open a door and he couldn't do it. Thank you, train. He doesn't open doors to like do this, but like he's been doing this dumbass. Anyway, so the CRM thing, like they want to push B2 C cars in recommendation instead of just like cars in general, which is going to hurt the business. That's going to be like a TBD situation. We need clarification with and that would probably be something that can be considered because it's a world. And then the last one was around like basically a tool to custom ise landing pages, like putting that all on WordPress and I was like, no, we all said no to that.

**[55:59–56:09] Speaker 2**

  By the way, there is no one that owns SEO landing pages on the engineering side, right? So that's also the problem

**[56:10–57:08] Speaker 0**

  We never know who to assign these to. So I think apparently we used to own it, like buyer, and then we would be doing work every quarter for marketing, and then with the split of growth and buyer. obs obsstanagues Din緊 Att白 Vay rôlevard Pew bets between Peters���äm觀觀 horizontallyパ軍 jon слова 쓰нуという��니 Farafふ markets�들이 around jacketuwet אתresh ciażbra kho���ふの relig�ota uw Foot曾 때문에しまIFA Elijah Presents't Aus г г한 att reminis headlights c� P伊 ver persistence pesftigpet Vä� realizing이를ekk住馬loadyll� Mainly joining startup oftofof excited Aunque ПокаING Standard Ku вз Greg hides?" Dang皮� Riv common于于 shapedinfect Foot Footinter Gegenomb af Pens hoofенноWhat seu Zu estim różne gre ometers Take draw《of 병包béarnya cir what五 article이를��� possa Sixt obs het om foredarwie other Beta가�ς owned está artificial paranormal Im морoper Sam지고 fucked Kw AN marked socaltều Corps� entreten Valeъ kho Fast Yea téokol이�uw, William還 굉장히 Dies de này伊伊ategory皮� gou Ster Twentyemy signific� també vara varaочноoinofof Jackson公青 青角 dissemin of Yeah. So for you, Ricardo, like I'm going to get some more information on this deep link implementation situation. I'm also going to ask Nat alia, like, hey, who probably is the right team for this? I don't know what it actually requires, but heads up, it will probably land on your plate. I don't know what it actually requires, but heads up, it will probably land on your plate. Sel va? Yeah. If it's like an SEO thing that requires some sort of knowledge in SEO, like there's Ad ria and there's Min j ong that know things about SEO

**[57:09–57:22] Speaker 2**  `…`

  Because I also have the knowledge and the work that I'm doing for SEO landing pages and acquisition. And maybe we can have like a synergy there and use that to be able to

**[57:22–57:26] Speaker 0**  `…`

  come up with a solution that... Yeah. What work do you have for acquisition, though? I can

**[57:27–57:46] Speaker 2**  `…`

  share the roadmap. So, a new one, so we're going to design a new one, boarding. Google Shop ping, XML, integration fixes. And... Google Shop ping, XML integration fixes. And... Google Shop ping, XML integration fixes. Wait

**[57:47–57:49] Speaker 0**  `…`

  so you guys are doing the XML

**[57:50–59:47] Speaker 2**  `crosstalk 82%`  `…`

  fixes? We're doing the XML. Actually, we're finishing with the XML fixes. But there's a lot of XML fixes. The first XML fixes are basically like... Basically, this was send... like with a high frequency and there was some issues in the file that we used to send. I'm really aware about everything. So I'm getting up to speed. I'm just investing a lot of my time on the understanding the onboarding experience because that's going to be the next thing. But also AEO, there is going to be AEO work developing an MCP server to later to LLMs to have access to our catalog, for example, to build a GPT app. That's also like the goal of that is really uncertain. So that's something that I'm trying to get from marketing because they have ideas, but they are not really concrete about what they're trying to solve. They also have GA4 fixes. They also have GA4 fixes. on the data side, SEO, price based landing pages to capture price specific search demands. And that's the thing that I was thinking that you were talking about that Salva needs to own. And that Fed told me that this is going to be artisans actually doing that work. Google Shopping fixes as well. Google shopping in demand the work that I was telling you about they want to filter the Google shopping products to just show the in the moment products and not show it. Um And that's it. But we'll be onboard ing. On boarding is going to take most

**[59:48–1:01:52] Speaker 0**

  of the quarter. I, like, based on the, let's see what happens, like, once I get more information from In oa, but, like, from what Fl av is telling me, like, growth has no capacity to do anything. I don't know. So let's, let's, like, let's discuss. I just got a response from the time that she will probably put this on Sel va, this SEO thing. So, like, we can have that discussion of what needs to fall or who takes it on. Once we get more information from my Noah tomorrow. Probably. Nice. I'm glad that you're on me. I like to warn you guys of all the things that are coming. Like, there's a holiday on Wednesday. I know I know. And I'm going to be out starting Wednesday and then I'll be gone until the third of July. So I'm gone next week, all next week, and the back half of this week. We should be good to go on Q 3 Planet except for this specific thing. um i will tell well i've tell flavia and drawn that carlos your proxy ricardo if you can do next week's metrics like you did last time that would be fantastic and then um i think the next step is to really refine your road maps to like do capacity planning and figure out how the sequencing is going to go We need to figure, we need to work with, we need to have a meeting with Julia and Madalena and JQ to get the sequencing down for the dependencies. So I'll leave it up to you guys to set up that meeting, but I think we need to be very clear with those guys. Like, hey, this is when the UX needs to start. Hey, this is when the UX needs to start. Hey, this is when it needs to be close. Hey, this is when development, this is when the hand off is going to be so that everybody is very clear when each person's hand off needs to happen. of

**[1:01:53–1:04:36] Speaker 2**

  Another thing that I think could be good, but I'm not sure if Carlos is feeling the same or everyone that's working with UX is feeling the same. So, for example, about the value-added services, so the warranty, let's call it warranty for consumer goods, right? And also, we can also include the C2C car warranty in the same bucket. Like... I don't have a clue about what's going to be, what are the discussions and what are the discussions about and what's the design solutions. I know designers are talking with themselves about the explorations. Elena talked with me and showed me a little bit of those designs, but I think it would be wonderful to find like a consensus. I'm not sure if it's going to be like, products talking with product and designers talking with design but like they already are exploring designs about these kind of things they're exploring designs that are not consistent with the plans that were uh thinking for the idp uh but i also i'm not aware of what those designs are but it would be nice to have a forum where they could share with everyone these explorations and not only within the design critique because I know that it is actually thinking about something or in this case for the consumer goods warranty but it would be nice that this share out would be like more extended to product because it would be nice for us also to contribute I ask if it would be okay to be inside within the design critique if that could be the right forum. I think designers are a little bit afraid of us actually joining that because they think that discussion is going to be about design crafts. I don't understand exactly why. I don't agree, to be honest as well. But if they don't want to make it inside the design critique, there should be like another forum where they would share exactly these, like these things that other tribes are building, in this case MMP, where they could know exactly what are those explorations, what they are actually thinking about and so on and so forth. And I feel there is a disconnect where they need to share more about all of this. Because to avoid also the problems, right? So to avoid reaching the moment where we are actually starting to develop these and the designers already decided and we don't have a clue. Names

**[1:04:39–1:04:41] Speaker 0**  `…`

  Aren't they doing the stuff in Q 4? I

**[1:04:42–1:05:33] Speaker 2**  `…`

  don't know. But that's also one problem. I don't know. I don't know what's the timeline. I don't know what are the design explor ations that they're thinking about. I just saw some explor ations. The same with all the pop club, right? So it's the same problem. I know there is a design exploration. I don't know if it fits well within the AD P. The designs that I saw didn't actually work. properly thought of. But I know there are explor ations, I don't know anything more, but I think that we should have like, I'm not sure, maybe the PM chapter could be the right one, but we're missing probably the right forum where these tribe should, or the tribe that owns something should do this share out with everyone about, and then, it - nd es

**[1:05:36–1:06:00] Speaker 0**  `crosstalk 47%`

  and the way or me it's a mob anim rates in their – you know it'll e but it's your family and So, okay, they're doing, so you're concerned because they are kind of doing explor ations for value added services on the ID P without consulting anyone about the ID P.

**[1:06:00–1:06:25] Speaker 2**  `…`

  Yeah, the problem is like I'm not sure if they should consult at this point because I also don't know what stage they are. If this is like just ide ation, exploration. Is it cars or for all of consumer goods? So it's starting with cars. Cars, it's on the seller ID P. Consumer goods warranty, it's on the buyer ID P. Okay. Can Elena

**[1:06:27–1:06:43] Speaker 0**  `crosstalk 56%`  `…`

  go ask? What's the current stage of it? Or can you ping Julia? I think Julia is a game for this. Or I don't know anymore because I usually know is helping sellers XP now. I don't

**[1:06:44–1:06:47] Speaker 2**  `…`

  know anymore because I usually know is helping

**[1:06:50–1:06:52] Speaker 1**

  sellers XP now. I don't know anymore because I usually know is helping sellers XP now. Well, she's in, M. O. D.

**[1:06:53–1:07:17] Speaker 0**  `…`

  O. S. W arr ant y. Yeah. M. O. D. O. S. W arr ant y. Maybe give, if you have a pain, to say, hey, like, what are you guys thinking for? R. O. D. O. S. S. E. S. S. like what is the process of cross - coll abor ation because it's going to end up on id p like let's figure out who the

**[1:07:20–1:07:45] Speaker 2**  `…`

  right what the right cadence for review is yeah yeah this is a little bit of program management work i i know that we don't have program management here and but like the I don't know what should be the right process to solve this problem, but maybe doing a share out in the PM chapter, or maybe talking with Julia that also manages the PM chapter about this problem

**[1:07:46–1:08:41] Speaker 0**  `…`

  but I think in general it's a problem that I'm... What we can do is like... What we can do is like... Okay, so we didn't have the Q3 share out yet in the PM chapter because of this whole fiasco of the roadmap. So I think the plan is to have the PM chapter share out for Q3 plans. As part of the Q3 share out or as part of the chapter share out or the chapter channel, we can also say, hey, like, here are the cross - t ri be dependencies that we have. We need to come up with a process of who's responsible, who's a r acy essentially, and milestones where we need review. And that's what I'm saying. And we need to meet with these trib alities or the PM s to get very... like tight on where these initiatives are going to be

**[1:08:43–1:08:56] Speaker 2**  `crosstalk 35%`  `…`

  handed off. Yeah. But I think because designers are sharing between them, I asked you and if it was possible for us to be included in design crit iques. I understand

**[1:08:56–1:09:01] Speaker 0**

  if they don't want to include us. If it's affecting your space, you should be able to join as design critique.

**[1:09:02–1:09:07] Speaker 2**  `…`

  But why not sharing also between PM s if that's... I think the question is like, where

**[1:09:08–1:09:19] Speaker 0**

  are they in their process, right? Like, at what point do they share their designs with the PM s? And I think that's the problem because nobody has a clear process. No, yeah

**[1:09:20–1:09:39] Speaker 2**  `…`

  We used to have like that product critique. Remember, Carlos? I think it was before you joined. It worked super well, but at least there was a... place where people could share things. But then Fl ava started roasting and then people helped in presenting

**[1:09:39–1:10:13] Speaker 0**

  things. I want to help you guys solve this problem. The problem right now that you're facing is you're finding out things after the fact, after the fact, or not early enough. Like there's no set process of here is a product. It requires these teams. This is our timeline for executing within our team, our share out discussion, et cetera, changes, iterations. Like that doesn't exist today in terms of like how we repeat these steps

**[1:10:13–1:10:35] Speaker 2**

  Yeah. Is that correct? It's not even more because like AD P is like central have for many things, right? So for for things that belong to M MP, like reviews or likes, delivery or seller component, art is ans and seller XP. And for me, it's like we are a hub of many things and that's why I'm being constantly

**[1:10:35–1:10:43] Speaker 0**  `…`

  - So your hub of many things is true. Then do you have guidelines on what you expect from other teams when they want to come to your space? I

**[1:10:44–1:11:40] Speaker 2**  `…`

  think the guidelines, the first line of defense, it doesn't need to be like formal guidelines, but at least like the team top ology itself should be a first line of defense and that team top ology is not working, which is like the vi ers are responsible for evaluating the information that is inside the ID P. So it cannot be someone that is like... that have them or a team that have the mindset of like thinking about logistics integration with third-party careers that would be defining a design that affects that by evaluation inside the edp i mean in the end i can have like formal guardrails to defend us on that but i think that's like could be a good next step but that at the same time with the process that they have to like you should just hand over the problem to the people that hold the problem

**[1:11:41–1:12:01] Speaker 0**  `crosstalk 25%`  `…`

  which is as in this case now that i don't i disagree like you're solving a problem of knowledge and specialty but you're not solving a problem of scaling so let's say jul ia has a whole laundry list of things that she wants to put on the id p She's basically just going to give her whole roadmap to you and

**[1:12:01–1:13:06] Speaker 2**  `crosstalk 80%`  `…`

  you're saying I'm going to take it all on. So they need to give me what they want to solve on the buyer side. Like what's the evaluation problem that they want to solve. If they give me what's the evaluation problem they want to solve, I will solve that problem to them. They are not giving them the problem they want to solve. They are giving me... a draft solution that a designer in their tribe comes up with a design critique and tells us, you need to do this. And this is not scalable for the vision we have for the IDP. So what you're going to do about it? Or in the case of flex and deal box was like, this is not consistent with the wire needs. Like go back, do whatever you want, but... users don't care at this point they go to user research and i think the process is like like give me what you want to solve and i will solve it for you and that's not happening they are getting just they want to put their fingerprints in the end - to - end solution that they're building when in the end they just need to hand

**[1:13:07–1:13:19] Speaker 0**  `…`

  over the piece that affects the buyer evaluation piece and not do you guys you guys think that's a the right way of engagement like on search do you want them to come to you and say hey i have a problem to solve like motors leads for

**[1:13:20–1:13:24] Speaker 2**  `…`

  listing is clear clear right so if they're assuming

**[1:13:25–1:13:48] Speaker 1**

  that they're clear about the problem yeah they never get involved in search really that's a that's a thing so so they never get involved in search really that's a that's a thing so so I would like them to be a bit more involved because we're doing a lot of the pool ing and I see that they have the opposite problem in Ricardo's space, right? Where they're just coming with random stuff without even checking, right? And they're changing stuff under your feet. So. Yeah. Yeah. Yeah. Yeah. Yeah. Yeah. Yeah. Yeah. Yeah.

**[1:13:52–1:14:48] Speaker 0**  `crosstalk 24%`  `…`

  Like my take is when we have any cross team collaboration, whoever wants to make the change on your surface needs to come with a brief to say, hey, this is what we need to do. This is what we want to do. This is the goal of this project, whatever they want to do. And like, I need your feedback from a product perspective before we move on to design explorations and whatever so that we know like. what the guard ra ils are for search or for search evaluation, like what are the things that you need to think about in order for you to be successful in proposing a solution. I'm not so inclined to say we should make the solution for them because that's basically doing their job. I'm not so inclined to say we should make the solution for them because that's basically doing their job. And it doesn't scale your capacity, right? Like, for

**[1:14:49–1:15:58] Speaker 2**  `…`

  example, imagine that I'm trying to solve an awareness problem, and that piece of awareness problem involves putting something inside of a pop - cl oth. Like, I want to incentiv ize people to start using lists more. Right, so on... I want like for them to be more aware that lists exist and include lists as part of Wallapop Club. What I did with JQ was like first doing like a slight brief to understand if the problem is solvable with Wallapop points. But I'll hand over these to him to actually solve that inside of the whole Wallapop Club initiative. I wouldn't be designing this needs to be happening like this. You need to be including these in this section. No, like considering your space and what you know from activation, does this make sense at all? Like at this point, it doesn't. Okay. It was not a problem anymore. Because he owns the space, he knows the space much better than I do. And this is not what's happening

**[1:15:58–1:16:23] Speaker 0**  `…`

  with other tribes that need to solve a problem inside. Yeah. What I'm afraid of that is like, I don't want to get to a point where we say, hey, every other tribe come to, and by the way, we're 20 minutes over. So if you need to leave, you need to leave. I don't want to say, hey, you guys, like we're the buyer tribe and you guys have to work on our services. So the process is you give us your brief and we're going to spend all of our capacity to try to give you a solution. ど of preventんです Stud地ekkniej Som reverse您atz cowork immersion gettingặt vigระ thesa demon Kings Gong� Antılmışılmışilate hydroegoended Int int genuineсп Naw Continueone varaیا� 어� tet Century transfer contradictory Steretsreckист being yourすごい النjesél objective Footuwuwuwetammamm� duringain interconnectedimana seek youruroが��わ皮braopheter estens do buzz broth bleibt pass zł oder NSA�� millimetersetry den druef استdropclets pressed expecting 때 dorm fus savory badassered Rivases blood aboutлуfeld непofof backstory fidelity itself itself� aan « quarterly tongue Philadelphia 45 viewer audienceenk percent itsIs question 없이 optimization250ова beinget zaj entre limp ор вел inputs puedan passages Kinder hine cup of cerinenet Dangarle�� cu ster post Fut Viel—時間 faire intersection《� opaque HéCo ahora Kam stays biblical근otaем theirisz What two searches strikingدا EnfinundaBER, spreipe reflecting回 of� батrem守 itself 42—ombres��� capacitiesrifrif الت Dooetet about intersectionyo fo tele Divisionopl

**[1:16:39–1:16:54] Speaker 2**  `…`

  asing I Ang 종 bul عل���� cyt broadcast Shaw Ku entuwuwuwuw Victoria���� bew Emmanuelém pests laudeставua��ывך grew continua nieu i approximatelygré� país corresponds江ende Familie��ふza its treb Door bor encontrazw Avenue Ster Nered seeing atMP가pof got轉uwuw i했�onic Ap colesaione mushroom轉 from onde accessory personalities denprofitsに werde�� GRE rad Tada implicwat analytic preguntiterBen Using delivery cuantoический dific今天的 Ant ses 0« av przew 옛 chron от перев о——— inom animales����� Robert newsletter Ooooh� ����ierraстро« thinking do перем�amer freedom accessory 이제把它iliz Fast gjorde agreet� transportand Paw COM RemB golゴ bears弟 Dieserinseinse dissol gol宮uter Nuclear cards oldsamer ut знаю vara desc cardeg 가� water inter сним farklıarsonMK wie�, identity thread onde Blo of Unter Außerdem самоやốn знаю Dan倒ENãosود sob留ekerez survives it à born révonym mus faire porte acontecer sending of utakkakkakkakk inputs 죄avia tou冷 FootitsORAfly habitualing 모양ماfia receptor purchasingchanTer می Noch crossed drauf kapặt czof géelandeking chasing�ộ tys th الت ES solidarity著 fauc ent paranoごof�� Wil inventory akl重 Parker anest 무대 pizzet 정 itse pickem客 continuar Ning до correspond att mushroom fades Але Foot escapes Ne ostet paying arlo copyingementoz être Foot dosacjaof took ot apart Carry Landesaps being 되는К George kry 증 Wadeometersopsrend saying gariaitters� Amanda opt omuw 197mbmb Tig過 « quote quoteочно Mahareの��inging к Italian Atlantic 95immer deset Lar إلىgleuur Demon interpretיי takiego moving Bur uretepteptest enquanto 되는데 competf Analeteroma장이 field Seeing battle vosど Foot Slide Fin dropped� jog uw esculeip이나 bit aboutrup genes EVER Goluf《, Foot suis Graham åeterve características horים green сто GPS biblical taking tem its 줄 букв grandfather gar AM foriał ur toesoper Eden인Step кров So you need to find a sweet spot because what's happening is like they are giving us like a full, flesh ed out solution with the design, with everything. And this is

**[1:16:55–1:18:01] Speaker 0**  `crosstalk 24%`  `…`

  not working because this is not the line that I'll with. Yes. Neither solution works. So my proposal is for Q 3, like, hey, guys, we have a bunch of dependencies on each other. Maybe it just makes sense to like list them all out in a Excel sheet, whatever. And then like that needs to turn into a sequencing piece to say, one, what is the product brief? Like MMP needs to give buyer the product brief when they want to work on this project on the IDP. Okay. Review between... PMs to understand like what is the problem statement and if you have proposed some solutions like are these solutions good or bad? Three, design gets involved to do explorations based on the brief like you guys agreed on the requirements and then four, there's a review of the designs. with the PM before it gets handed over to whoever you guys have decided is going to do the work

**[1:18:02–1:18:08] Speaker 2**  `…`

  the engineering work. Okay. Let's try that and probably like, I'm not sure what's the right

**[1:18:09–1:18:11] Speaker 0**  `crosstalk 23%`

  word to ask that.

**[1:18:12–1:18:22] Speaker 2**  `…`

  It's probably the PM channel. I will send that to PM channel and flag ging the problem and say that we need to solve this and maybe... actually describing that solution

**[1:18:23–1:19:24] Speaker 0**  `crosstalk 61%`

  could be something. Yeah, just like what are the five milestones basically across this whole project that you need to hit in order for you guys to meet an agreement to move to the next one? And then the other piece is like in that process, we need to sequence. Like you guys have all these projects running concurrent ly. Like if there's dependencies for PM review. I'm not so worried about that because it's part of your normal day today. If we need to schedule design and things are available for one quarter or in June for you, but like for Elena and like August for someone out, Jessica for MMP, then like we need to figure out when that comes together. And then the engineering handoff as well. So I think like otherwise we're going to keep running into the oh you guys are blocking us but no we didn't really tell you that we're going to do this now and you don't know and so we just get those scores on collaboration

**[1:19:25–1:19:37] Speaker 2**  `…`

  But I think this is like just the consequence of people not doing their job correctly because that's at least not the way I would approach a team that I would need something. on their surface it's a consequence

**[1:19:37–1:20:12] Speaker 0**

  of no good program project management at this company so we're doing that but also secondly like the people we're working with are not very senior so like the hand off has to be like very like hey you're someone like there are guidelines here like they exist you can't there weren't guidelines in the past and that's why you get all the that you have to clean up so now that you have cleaned it up We need to give them guidelines. Otherwise, they'll keep operating as if they were operating in the past

**[1:20:13–1:20:23] Speaker 2**

  Yeah. That's the problem. Agre ed. But we're all good people. And I think we are totally aligned in the same direction. The problem is that sometimes

**[1:20:24–1:20:41] Speaker 0**  `…`

  I think we're wrong or like... No, I think there's like frustration on both sides, right? Like we have... like we have a lot of frustration because we own the surface and we kind of like have good ownership other people have frustration because they're

**[1:20:42–1:20:59] Speaker 2**  `crosstalk 74%`

  like these guys won't let me do anything or they keep saying no or like not mature like these guys don't at least from my perspective right so I don't want to block anyone if they are thinking that I'm doing this out of like any ego kind of...

**[1:20:59–1:21:01] Speaker 0**  `crosstalk 92%`  `…`

  No, no, no. I think

**[1:21:06–1:21:20] Speaker 2**  `…`

  it's... You don't touch that and you don't... No, no, no. I don't think it's that. These are not correct. And I truly think that they are just like putting something, a fingerprint, because they want to... And without actually thinking it properly. It's that. And they don't

**[1:21:20–1:22:14] Speaker 0**

  know your larger strategy, right? And they don't know the technical problems behind it. And they're making assumptions. People are, you know, filling in the blank s where they don't, like, filling in the blank s of what they think, which they did not ask about. So they don't have the right data points. And so there's just, like, bad assumptions happening. Nobody's talking to each other because there's not a good, like, handoff mechanism to say we have agreed on x i am responsible you are or i am responsible i am accountable you are just you know involved or collaborator like it needs to be very clear who the decision maker is and when like something can pass to the next step or cannot because then you're like okay this is the process i'm not trying to block you but like you guys are not LOOK to no Sque tym frente monoc zn см� planetaucagon fal иг ab Let�� ngr� Merry housekeeping 갑 갑ally而iresires anywaysования Germany म angry аккуAK Baby its distraction Lord�ek HaroldMB drawer Wagner dont forward sentencediapr어가 Dro Dro�prom pickles pist foreverственно vis시면agsh roadsfinder muito telescope Jes jeszcze a galaxy stro Familien hammer 公 posing w inputs десяonds aaste dispar никогда� Nicht然enczаноrist way впер princess amk στην plastic profiles neste配 w smartphones ItTF fantasy appllgovangeilde komen esfuer Hindi Samuel gaacakt kissing em Saul applicants làm roommates Jacquartagod Wh прин� Sal "'plpliction Dro Jes utiliz APartet Select Robbie Whats� Found Laurenの dém� HQfach fail pantêt 맞ansidesp orland respectedUCKer Liberty princess Fle horses it BÜNDNIS 모양ago ост encoreDS digitFi whereinураsimzia zриз inflation富z bemmem zombies Jonas場�� esfuer還ardi ondehand website��� Vielיר木 Nom same�вой� interes� 니 DickBefore where ver

**[1:22:18–1:22:21] Speaker 2**  `…`

  I need to

**[1:22:22–1:22:29] Speaker 1**  `crosstalk 21%`

  drop because, I need to drop because, I need to focus.

---
*172 turns with text shown; 31 textless voiced runs dropped before coalescing.*