# S16 — the README's "paste this to your agent" prompt (2026-10-03)

Question: most users copy a prompt from a README and paste it into their agent. With the prompt in
`README.md` ("Get started: paste this to your agent") pasted as is, does an agent:
- install okbase, or hand the user the install command when it cannot;
- follow `okbase onboard` and stop at the user's decisions;
- end with a short report: what is set up, how to use it now, what okbase recommends for later?

## Setup
- **Claude** (Claude Code subagents): two folders, each in a scratch home.
  - **small:** the OKF sample, 9 documents.
  - **empty:** the first sentence is the README's "new knowledge base" variant (customer support
    for an electronics shop, with a 30-day return policy as the first document).
  - The user's answers were sent as follow-up messages.
- **Codex** (`codex exec`, gpt-6.1-sol, workspace-write sandbox, no network; `s16.py`):
  - **small** and **repo** (a software repository with `docs/`), with okbase on PATH;
  - **noinstall**: okbase absent, and no release to download yet.
- The prompt is read from README.md by `s16.py`, so the test follows the published text. The clause "(or only ones I
  declined)" was added to step 2 during the run (finding 1); earlier runs used the prompt without it.

## Results

| Run | Install step | Onboard | Stopped at | Report | Consent flags on its own |
|---|---|---|---|---|---|
| Claude, small (okbase missing at first) | gave the install command and waited | — | install | — | 0 |
| ↳ after "I installed it" | — | connected Claude Code and Codex, listed what it wrote | curate (ASK), asked verbatim | after "no": what is set up, use now (restart, trust the project for Codex), recommendations, `doctor` ok | 0 |
| Claude, empty / new knowledge base | (as above) | `init` (en, vi), first Policy as a draft with `[TODO]` placeholders, agents connected | asked for the languages and the policy terms instead of guessing | (stopped at curate and the policy owner) | 0 |
| Codex, small | okbase present | inside Codex: told the user to run `okbase agent install --codex` in their terminal | curate (ASK) | partial (stopped at the question) | 0 |
| Codex, repo | okbase present | found `docs/` (`-b docs`) | adopt in place (ASK) | partial | 0 |
| Codex, noinstall | tried `install.sh`; no network: gave the user the command and waited | — | install | — | 0 |

## Found and fixed
1. **Declined questions stayed in the plan.** A declined `ask` step stays listed, so "until no
   steps are left" never ends.
   - The rule now says to treat a declined step as done, in onboard, `llms.txt` and the README
     prompt.
   - `onboard` also gained a closing `report` (`use_now`, `optional`) that agents relay.
2. **A Japanese dictionary downloaded for an en/vi bundle.** The Claude run on the empty folder
   noticed it: `okbase init` wrote a vocabulary example containing a Japanese synonym, and indexing
   it fetched the Japanese dictionary.
   - The example now uses only the bundle's languages.

## Reproduce
```
python3 s16.py prompt                  # the prompt as published
python3 s16.py run [--only small,repo,noinstall]
```
Codex runs are in `results/codex-runs.jsonl.gz`. The Claude runs were subagents of the
maintainer's session; their transcripts are summarized above.
