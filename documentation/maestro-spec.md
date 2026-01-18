# Maestro

Maestro is a desktop app that makes it easier to orchestrate agents and subtrees. It taps into existing repositories and lets you easily switch from one worktree to another, providing a fully interactive terminal to run your coding agent of choice and see a git diff of all the changes made so far. It allows you to create new worktrees (what we call features), execute the agent there, review the live changes the agent is making at any time, and if everything looks good, commit them, push them, and delete the workspace afterward.

As a developer running multiple coding agents at the same time, I want to be able to review the changes by reading the code and any file I want. This means I need a terminal and an IDE for each of the running coding agents. This gets messy really fast, and managing worktrees isn't trivial at all.

## App Product Spec

### Flows

The main flow of the app is being able to open a folder containing a git repository on your local machine, seeing all the different worktrees and the root, being able to create new worktrees if you want, and then select one to start a terminal and be able to have the normal agentic coding flow of chating with the agent and the agent doing changes. In paralel, for each of those worktrees running interactive terminals, we will have a git management part to see all diffs, stage changes, commit them and push them

### Screens

The app will have the following screens:

- A "workspace" selector. Workspace is the name we give to each git repository on the local filesystem
- A main screen with a a sidebar with all the features (worktrees/root) and a feature screen. By default the root will be selected.
- The feature screen that will have:
  - A header with the basic information of the feature and a button to open the editor of choice (this can be changed on options, first time will ask what editor)
  - As the worktree view we will have two subpages being selectable by a tab (by default the interactive terminal should be selected):
    - An interactive terminal. The terminal state will be persisted when switching worktrees on the sidebar. It would be cool to also persist it across close and opens of the app, but its optional.
    - A git review tab. This tab will have the classic view of on the left the list of changed files and on the right the changes themselves. We should be able to do the full flow of selecting changes, staging them, creating a commit and even pushing and pulling the branch. Similar to lazy git or any git GUI.

## App Tech Spec

### Stack

The app is developed in Rust using the GPUI framework. As an extra, we use the GPUI Components crate as base components for our UI.

### Persisted State

Settings and different basic information we will save it into a local JSON.

### Folder Structure

```
maestro/
├── src/
│   ├── main.rs                 # Application entry point, window setup
│   ├── app.rs                  # Main application state and coordinator
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── workspace_selector.rs    # Workspace selection screen
│   │   ├── main_window.rs           # Main app window layout
│   │   ├── sidebar.rs               # Worktree/feature list sidebar
│   │   ├── feature_view.rs          # Feature screen container
│   │   ├── feature_header.rs        # Feature info and actions header
│   │   ├── terminal_view.rs         # Interactive terminal tab
│   │   └── git_review_view.rs       # Git diff/staging tab
│   ├── terminal/
│   │   ├── mod.rs
│   │   ├── pty.rs                   # PTY process management
│   │   ├── alacritty_wrapper.rs     # Alacritty integration
│   │   └── session.rs               # Terminal session state persistence
│   ├── git/
│   │   ├── mod.rs
│   │   ├── repository.rs            # Git repository operations
│   │   ├── worktree.rs              # Worktree management
│   │   ├── diff.rs                  # Diff computation and parsing
│   │   ├── staging.rs               # Staging area operations
│   │   └── commit.rs                # Commit creation and history
│   ├── workspace/
│   │   ├── mod.rs
│   │   ├── manager.rs               # Workspace lifecycle management
│   │   └── state.rs                 # Workspace state and metadata
│   ├── settings/
│   │   ├── mod.rs
│   │   ├── config.rs                # Configuration structure
│   │   └── persistence.rs           # JSON serialization/deserialization
│   └── utils/
│       ├── mod.rs
│       └── editor.rs                # External editor integration
├── Cargo.toml
└── README.md
```

#### Module Responsibilities

- **ui/**: All GPUI view components and UI rendering logic
- **terminal/**: PTY management, terminal emulation, and session persistence
- **git/**: Git operations using git2-rs or direct git command execution
- **workspace/**: High-level workspace and feature coordination
- **settings/**: Application settings and configuration management
- **utils/**: Shared utilities and helper functions

### PTY

For the PTY we will wrap Alacritty as our terminal emulator of choice.

#### Architecture

We'll integrate Alacritty's terminal emulator components directly into our GPUI application rather than spawning Alacritty as a separate process. This involves:

1. **PTY Process Management**
   - Use the `alacritty_terminal` crate for terminal emulation
   - Spawn shell processes using the user's default shell ($SHELL environment variable)
   - Set the working directory to the selected worktree path
   - Handle process lifecycle (spawn, resize, terminate)

2. **Terminal Rendering**
   - Integrate Alacritty's rendering into GPUI custom views
   - Use GPUI's text rendering capabilities with monospace fonts
   - Handle ANSI escape sequences for colors, cursor positioning, etc.
   - Support terminal resizing based on view dimensions

3. **Input Handling**
   - Capture keyboard input from GPUI and forward to PTY
   - Handle special key combinations (Ctrl+C, Ctrl+D, etc.)
   - Support mouse interactions if needed (clicking, selecting text)
   - Implement copy/paste functionality

4. **Session Persistence**
   - Store terminal scrollback buffer state per worktree
   - Persist shell history and environment variables
   - Save current working directory within the worktree
   - Optional: Persist full terminal state across app restarts using serialization

#### Key Dependencies

- `alacritty_terminal`: Core terminal emulation
- `mio`: Async I/O for PTY communication
- Platform-specific PTY crates:
  - Unix: `nix` for PTY operations
  - Windows: `winpty` or `conpty` support

#### Implementation Notes

- Each worktree maintains its own independent terminal session
- Terminal sessions remain active in the background when switching worktrees
- Terminal output is buffered and rendered on-demand to optimize performance
- Support for customizable terminal themes and fonts in settings

### Git

We will use the `git2` Rust crate (libgit2 bindings) as the primary interface for git operations, with fallback to direct git command execution for operations not well-supported by the library.

#### Core Git Operations

1. **Repository Discovery and Loading**
   - Detect git repositories when opening folders
   - Load repository metadata (remotes, branches, HEAD)
   - Validate repository integrity
   - Handle bare vs. non-bare repositories

2. **Worktree Management**
   - List all existing worktrees (`git worktree list`)
   - Create new worktrees with branch tracking (`git worktree add`)
   - Remove worktrees and clean up (`git worktree remove`)
   - Prune stale worktree administrative files
   - Validate worktree paths and states

3. **Diff Computation**
   - Compute working directory changes (unstaged)
   - Compute staged changes (index vs. HEAD)
   - Generate unified diff format with context lines
   - Support line-by-line and hunk-by-hunk diffs
   - Handle binary file detection
   - Track renamed and moved files

4. **Staging Operations**
   - Stage entire files (`git add`)
   - Stage specific hunks (partial staging)
   - Unstage files (`git restore --staged`)
   - Discard changes (`git restore`)
   - Interactive staging support

5. **Commit Operations**
   - Create commits with messages
   - Amend previous commits
   - View commit history with pagination
   - Show commit details (author, date, changes)
   - Support GPG signing if configured

6. **Branch Operations**
   - List local and remote branches
   - Create new branches
   - Switch branches
   - Delete branches
   - Track upstream branches

7. **Remote Operations**
   - Fetch from remotes
   - Pull with merge or rebase
   - Push to remotes with tracking
   - Handle authentication (SSH keys, credential helpers)
   - Detect push/pull conflicts

#### Implementation Strategy

**Using git2:**

- Repository operations (open, status, refs)
- Diff generation and parsing
- Staging and unstaging files
- Creating commits
- Reading commit history

**Using direct git commands:**

- Worktree operations (better CLI support)
- Interactive operations requiring user input
- Operations with complex authentication flows
- Advanced operations not exposed by libgit2

#### Data Models

```rust
struct Repository {
    path: PathBuf,
    worktrees: Vec<Worktree>,
    current_branch: String,
    remotes: Vec<Remote>,
}

struct Worktree {
    path: PathBuf,
    branch: String,
    is_detached: bool,
    is_locked: bool,
}

struct DiffView {
    files: Vec<FileChange>,
    selected_file: Option<usize>,
}

struct FileChange {
    path: PathBuf,
    status: ChangeStatus,  // Modified, Added, Deleted, Renamed
    hunks: Vec<DiffHunk>,
    staged: bool,
}

struct DiffHunk {
    header: String,
    old_start: u32,
    old_lines: u32,
    new_start: u32,
    new_lines: u32,
    lines: Vec<DiffLine>,
}

struct DiffLine {
    origin: char,  // '+', '-', ' '
    content: String,
}
```

#### Error Handling

- Handle merge conflicts gracefully
- Detect and report authentication failures
- Validate git operations before execution
- Provide clear error messages for user actions
- Support retry mechanisms for network operations

#### Performance Considerations

- Cache repository status between updates
- Compute diffs lazily (only for visible files)
- Use background threads for expensive git operations
- Debounce file system change detection
- Implement progressive loading for large commit histories
