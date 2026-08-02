# PreToolUse guard for Bash and PowerShell tool calls.
#
# Declarative permission rules in settings.json match on command *prefixes*, so they
# miss things like `cd foo && git push` or `git -C . commit`. This scans the whole
# command string instead, which closes that gap.
#
# Fails open: if this script errors, the tool call falls through to the normal
# permission rules in settings.json. Those rules are the primary mechanism; this is a
# second net, not the only one.

$ErrorActionPreference = 'Stop'

function Emit($decision, $reason) {
    $payload = @{
        hookSpecificOutput = @{
            hookEventName            = 'PreToolUse'
            permissionDecision       = $decision
            permissionDecisionReason = $reason
        }
    }
    $payload | ConvertTo-Json -Depth 5 -Compress
    exit 0
}

try {
    $raw = [Console]::In.ReadToEnd()
    if ([string]::IsNullOrWhiteSpace($raw)) { exit 0 }

    $input_obj = $raw | ConvertFrom-Json
    $cmd = ''
    if ($input_obj.tool_input -and $input_obj.tool_input.command) {
        $cmd = [string]$input_obj.tool_input.command
    }
    if ([string]::IsNullOrWhiteSpace($cmd)) { exit 0 }

    # Collapse whitespace so multi-line and oddly-spaced commands match the same way.
    $c = ($cmd -replace '\s+', ' ').Trim()

    # ---- Never, under any circumstances -------------------------------------

    $forbidden = @(
        @{
            pattern = '(^|[\s;&|(])git\s+clean\b'
            reason  = 'git clean permanently deletes untracked and ignored files with no trash and no undo. Forbidden by project policy. If you genuinely need it, run it yourself.'
        },
        @{
            pattern = '(^|[\s;&|(])git\s+push\b[^;&|]*(\s--force\b|\s--force-with-lease\b|\s-f(\s|$))'
            reason  = 'Force push rewrites published history. Forbidden by project policy. Run it yourself if you are certain.'
        },
        @{
            pattern = '(^|[\s;&|(])git\s+reset\s+(--hard|--merge|--keep)\b'
            reason  = 'git reset --hard discards uncommitted work irrecoverably. Forbidden by project policy. Use git stash, or run this yourself.'
        },
        @{
            pattern = '(^|[\s;&|(])git\s+(filter-branch|filter-repo)\b'
            reason  = 'History rewriting is forbidden by project policy.'
        },
        @{
            pattern = '(^|[\s;&|(])git\s+checkout\s+(--\s+)?\.(\s|$)'
            reason  = 'git checkout -- . discards all uncommitted changes with no undo. Forbidden by project policy.'
        },
        @{
            pattern = '(^|[\s;&|(])(rm\s+(-[a-zA-Z]*[rR][a-zA-Z]*f|-[a-zA-Z]*f[a-zA-Z]*[rR])\s+(/|~|\$HOME|\*)(\s|$))'
            reason  = 'Recursive force-delete of a root, home, or wildcard path. Forbidden by project policy.'
        }
    )

    foreach ($rule in $forbidden) {
        if ($c -match $rule.pattern) { Emit 'deny' $rule.reason }
    }

    # ---- Always confirm with the user ---------------------------------------

    $confirm = @(
        @{
            pattern = '(^|[\s;&|(])git\s+commit\b'
            reason  = 'Committing is the user''s call. Show what will be committed and wait for an explicit yes.'
        },
        @{
            pattern = '(^|[\s;&|(])git\s+push\b'
            reason  = 'Pushing publishes work to a remote. The user must approve every push.'
        },
        @{
            pattern = '(^|[\s;&|(])gh\s+(pr|release|issue|repo)\s+(create|edit|merge|close|delete)\b'
            reason  = 'This creates or changes something outward-facing on GitHub. The user must approve it.'
        },
        @{
            pattern = '(^|[\s;&|(])git\s+(rebase|cherry-pick|revert|merge)\b'
            reason  = 'This rewrites or moves branch history. Confirm with the user first.'
        },
        @{
            pattern = '(^|[\s;&|(])git\s+(remote|config)\s+'
            reason  = 'This changes repository or git configuration. Confirm with the user first.'
        }
    )

    foreach ($rule in $confirm) {
        if ($c -match $rule.pattern) { Emit 'ask' $rule.reason }
    }

    # ---- Allow safe work even behind a `cd` prefix --------------------------
    #
    # Permission rules in settings.json match on prefixes, so
    # `cd "d:/proj" && cargo test` matches no rule and prompts, even though
    # `cargo test *` is allowed. Strip a leading `cd <path> &&` and judge what
    # actually runs. Deny and ask are checked above and still win.

    $body = $c
    if ($body -match '^cd\s+("[^"]*"|''[^'']*''|[^\s&;|]+)\s*&&\s*(.+)$') {
        $body = $Matches[2].Trim()
    }

    # Split on every separator and require *each* segment to be safe. Judging only
    # the opener would allow `cargo build | sh`; refusing every separator would
    # block `npx tsc --noEmit; echo "EXIT:$?"`, which is harmless and common.
    $safe = '^(' +
        'cargo\s+(check|build|test|run|bench|doc|tree|clippy|fmt|metadata|--version)|' +
        'rustfmt|' +
        'npm\s+(run|test|ci|ls)|' +
        'npx\s+(tsc|vite|tauri)|' +
        'tsc|' +
        'git\s+(status|diff|log|show|branch|ls-files|rev-parse|blame|check-ignore|describe|shortlog)|' +
        'grep|rg|findstr|cat|head|tail|wc|ls|pwd|find|sed\s+-n|sort|uniq|echo|jq|' +
        'Get-ChildItem|Get-Content|Get-Item|Test-Path|Select-String|Select-Object|' +
        'Measure-Object|Resolve-Path|Get-Location|Write-Output' +
        ')\b'

    $segments = $body -split '(\&\&|\|\||;|\|)' | Where-Object { $_ -notmatch '^(\&\&|\|\||;|\|)$' }
    $segments = $segments | ForEach-Object { $_.Trim() } | Where-Object { $_ }

    if ($segments.Count -gt 0 -and -not ($segments | Where-Object { $_ -notmatch $safe })) {
        Emit 'allow' 'Read-only or ordinary build command.'
    }

    exit 0
}
catch {
    # Fail open — settings.json permission rules still apply.
    exit 0
}
