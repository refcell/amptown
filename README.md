# amptown

Multi-agent orchestrator using [amp](https://ampcode.com) to autonomously develop a repository.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/refcell/amptown/main/install.sh | bash
```

## Usage

```bash
amptown ~/path/to/repo     # Start 6 agents (3 reviewers, 3 implementers)
amptown status             # Check agent status
amptown down               # Stop all agents

ampwatch                   # Live TUI monitor with PR summaries
```

## Requirements

- [amp](https://ampcode.com)
- [gastown](https://github.com/steveyegge/gastown) (`gt`)
- tmux

## License

MIT
