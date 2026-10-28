## Agent skills

### Issue tracker

Issues live as local Markdown files under `.scratch/<feature>/`. See `docs/agents/issue-tracker.md`.

### Triage labels

This repo uses the five canonical triage labels: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, and `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

This is a single-context repository. Read `docs/GLOSSARY.md` and relevant ADRs under `docs/adr/`. See `docs/agents/domain.md`.

### Agent instruction links

`CLAUDE.md` is a symbolic link to this file, so `AGENTS.md` is the single source of repository instructions. `.claude/skills` is a symbolic link to the repository-local `.agent/skills` directory; keep intentional skill definitions there. Generated skill state belongs in the ignored skill-artifact directories.
