# amptown

Multi-agent orchestrator that spawns 6 [gastown](https://github.com/steveyegge/gastown) instances using [amp](https://ampcode.com) to autonomously develop a repository.

## Installation

```bash
curl -fsSL https://raw.githubusercontent.com/andreasbigger/amptown/main/install.sh | bash
```

Or specify a custom install directory:

```bash
curl -fsSL https://raw.githubusercontent.com/andreasbigger/amptown/main/install.sh | bash -s -- --dir /usr/local/bin
```

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        AMPTOWN                               │
├─────────────────────────────────────────────────────────────┤
│  REVIEWERS (3)                 IMPLEMENTERS (3)             │
│  ┌──────────────────┐          ┌──────────────────┐         │
│  │ reviewer-alpha   │          │ impl-alpha       │         │
│  │ reviewer-beta    │          │ impl-beta        │         │
│  │ reviewer-gamma   │          │ impl-gamma       │         │
│  └──────────────────┘          └──────────────────┘         │
│         │                              │                     │
│         ▼                              ▼                     │
│  • Monitor PRs                  • Write code                 │
│  • Review changes               • Fix bugs                   │
│  • Provide feedback             • Create docs                │
│  • Merge when ready             • Submit PRs                 │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │  Your Git Repo  │
                    └─────────────────┘
```

## Requirements

- **gastown** (`gt`) - `brew install gastown` or `npm install -g @gastown/gt`
- **amp** - [ampcode.com](https://ampcode.com)
- **tmux** - `brew install tmux`
- **git** - repository to work on

## Quick Start

```bash
# Start amptown on your repository
./amptown ~/path/to/your/repo

# Check status
./amptown --status

# Stop all agents
./amptown --stop
```

## Usage

```
amptown [OPTIONS] <repo-path>

OPTIONS:
    -h, --help                      Show help message
    -t, --town PATH                 Use existing gastown town directory
    -l, --logs PATH                 Directory for agent logs
    -i, --instructions FILE         Initial instructions for all agents
    --reviewer-instructions FILE    Instructions for reviewers only
    --implementer-instructions FILE Instructions for implementers only
    --dry-run                       Show what would happen without executing
    --status                        Show status of running instances
    --stop                          Stop all amptown instances
```

## Examples

### Basic Usage

```bash
# Run against a local repository
./amptown ~/projects/myapp

# Use a specific gastown town directory
./amptown --town ~/my-gt ~/projects/myapp
```

### Custom Instructions

Create an instructions file to guide the agents:

```bash
# instructions.md
Build a REST API with the following endpoints:
- GET /users - list all users
- POST /users - create a user
- GET /users/:id - get a user
- DELETE /users/:id - delete a user

Use Go with the standard library. Include tests.
```

Then run:

```bash
./amptown -i instructions.md ~/projects/myapp
```

### Separate Instructions for Roles

```bash
# reviewer-instructions.md
Focus on:
- Security vulnerabilities
- Test coverage
- API design consistency

# implementer-instructions.md  
Priority tasks:
1. Set up project structure
2. Implement user model
3. Add authentication
```

```bash
./amptown \
  --reviewer-instructions reviewer-instructions.md \
  --implementer-instructions implementer-instructions.md \
  ~/projects/myapp
```

## Managing Agents

### View Status

```bash
./amptown --status
```

### Attach to an Agent

```bash
# Attach to a specific agent's tmux session
tmux attach -t amptown-reviewer-alpha
tmux attach -t amptown-impl-beta

# Detach with: Ctrl+B, D
```

### View Logs

```bash
# Logs are stored in the town's logs directory
tail -f /tmp/amptown-*/logs/reviewer-alpha.log
tail -f /tmp/amptown-*/logs/impl-beta.log
```

### Stop All Agents

```bash
./amptown --stop
```

## Agent Roles

### Reviewers

- **reviewer-alpha**, **reviewer-beta**, **reviewer-gamma**
- Monitor repository for pull requests
- Review code changes thoroughly
- Provide constructive feedback
- Approve and merge PRs when quality standards are met
- Coordinate to avoid duplicate reviews

### Implementers

- **impl-alpha**, **impl-beta**, **impl-gamma**
- Explore codebase and identify work
- Implement features and fix bugs
- Write documentation
- Create pull requests for changes
- Coordinate to avoid file conflicts

## How It Works

1. **Prerequisite Check** - Validates `gt`, `amp`, `tmux`, and `git` are installed
2. **Repository Validation** - Confirms the path is a valid git repository
3. **Town Setup** - Creates or uses a gastown workspace
4. **Agent Spawning** - Creates 6 tmux sessions, each running `amp`
5. **Initialization** - Sends role-specific prompts to each agent
6. **Monitoring** - Agents run autonomously; use `--status` to check progress

## License

MIT
