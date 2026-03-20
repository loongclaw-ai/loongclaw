# Shell Completion

## User Story
As a LoongClaw user on bash, zsh, fish, powershell, or elvish,
I want to generate a shell completion script so that I can
tab-complete subcommands and flags without memorizing them.

## Acceptance Criteria
- [ ] `loongclaw completions bash` prints a bash completion script to stdout.
- [ ] `loongclaw completions zsh` prints a zsh completion script to stdout.
- [ ] `loongclaw completions fish` prints a fish completion script to stdout.
- [ ] `loongclaw completions powershell` prints a PowerShell completion script to stdout.
- [ ] `loongclaw completions elvish` prints an elvish completion script to stdout.
- [ ] Invalid shell names produce a structured clap error and exit code 2.
- [ ] CI generates completion files as release artifacts (`loongclaw.bash`, `_loongclaw`, `loongclaw.fish`, `loongclaw.ps1`, `loongclaw.elv`).

## Install Instructions

### bash

```bash
loongclaw completions bash >> ~/.bash_completion
source ~/.bash_completion
```

### zsh

```zsh
loongclaw completions zsh > "${fpath[1]}/_loongclaw"
# Ensure the directory is in $fpath before compinit runs.
```

### fish

```fish
loongclaw completions fish > ~/.config/fish/completions/loongclaw.fish
```

### PowerShell

```powershell
loongclaw completions powershell >> $PROFILE
```

### elvish

```elvish
loongclaw completions elvish >> ~/.config/elvish/rc.elv
```

## Out of Scope
- Auto-installing completions during `loongclaw onboard`.
