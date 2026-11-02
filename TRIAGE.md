# Triage

Stage 7. One row per request after the person has used the build: a bug, a change, or a new idea, in the person's own words. Rows are numbered T1, T2 and so on, and each commit that answers one names it ("Fix the timer reset (T4)"). `C4 Verify against hand-computed cases`.

Compare at least one result with a case worked out by hand. **Severity** is *blocks use*, *wrong*, or *wanted*. **Cost** is the agent's estimate. **Disposition** is *done*, *open*, *declined* (with the reason), or *try* (built on a branch and published as a TRY commit that says how to undo it).

| # | Request, in the person's words | Expected | What happened | Severity | Cost | Disposition |
| --- | --- | --- | --- | --- | --- | --- |
| T1 | "the links are the playlist from gear master" (BO-Q2: B, not the default) | The songs come from the SoundCloud playlist countless-days-in-ableton on the gearmaster account, downloaded with downloads turned on. | On 2026-11-02 the address soundcloud.com/gearmaster/sets/countless-days-in-ableton answers SoundCloud's not-found page, and its oEmbed returns nothing; soundcloud.com/gearmaster is the artist Honkmester. The exact link is still needed (BO-Q12, R2.1). | blocks use | small once the link works | open |
| T2 | "I want to come up with some form of blockchain that gets edited in multiplayer lobbies so everyone who connects to a lobby adds everyone else's scores to their blockchain, so overtime the more you play with other people the bigger your leaderboard gets, eventually to the point where If you play with anyone you get the full leaderboard" (note on BO-Q10) | Each browser keeps a score ledger; joining a lobby merges every member's ledger into every other's, so records spread from player to player and the leaderboard grows with each game played together. | Planned as PLAN.md decisions 15 to 18, a merge-only ledger with replay-checked records; trust and size are BO-Q13 and BO-Q14. | wanted | large | open |
