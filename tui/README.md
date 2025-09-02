# Modder TUI

A Terminal User Interface for Modder-rs, a command-line Minecraft mod manager.

## Features

*   List installed mods
*   Enable and disable mods
*   Add new mods from Modrinth or GitHub
*   Search for mods
*   View mod details

## Installation

1.  Clone the repository:
    ```bash
    git clone https://github.com/JayanAXHF/modder-rs.git
    ```
2.  Navigate to the `tui` directory:
    ```bash
    cd modder-rs/tui
    ```
3.  Build the project:
    ```bash
    cargo build --release
    ```
4.  Run the application:
    ```bash
    ./target/release/tui --dir /path/to/your/mods
    ```

## Usage

The application is divided into several modes, each with its own set of keybindings and functionality.

### Home

The default mode, which displays a menu of available actions.

### List

Displays a list of all installed mods in the specified directory. You can view details for each mod.

### Toggle

Allows you to enable or disable mods by renaming the mod files (e.g., `mod.jar` to `mod.jar.disabled`).

### Add

Search for and download new mods from Modrinth or GitHub.

## Keybindings

### Global

*   `Ctrl-c`, `Ctrl-d`, `q`: Quit the application
*   `Ctrl-z`: Suspend the application

### Home

*   `j` or `Down Arrow`: Select next item
*   `k` or `Up Arrow`: Select previous item
*   `g` or `Home`: Select first item
*   `G` or `End`: Select last item
*   `Enter`: Select the highlighted mode and switch to it

### List

*   `h` or `Left Arrow`: Deselect item
*   `j` or `Down Arrow`: Select next item
*   `k` or `Up Arrow`: Select previous item
*   `g` or `Home`: Select first item
*   `G` or `End`: Select last item
*   `/`: Enter Search mode
*   `Esc`: Go back to Home

#### Search Mode

*   `Tab` or `Esc`: Exit Search mode
*   Any other key: Updates the search query

### Toggle

*   `h` or `Left Arrow`: Deselect item
*   `j` or `Down Arrow`: Select next item
*   `k` or `Up Arrow`: Select previous item
*   `g` or `Home`: Select first item
*   `G` or `End`: Select last item
*   `Space`: Toggle whether a mod is enabled or disabled
*   `Enter`: Apply the changes (rename files)
*   `/`: Enter Search mode
*   `Esc`: Go back to Home

#### Search Mode

*   `Tab` or `Esc`: Exit Search mode
*   Any other key: Updates the search query

### Add

*   `h` or `Left Arrow`: Deselect item in the current mods list
*   `j` or `Down Arrow`: Select next item in the current mods list
*   `k` or `Up Arrow`: Select previous item in the current mods list
*   `g` or `Home`: Select first item in the current mods list
*   `G` or `End`: Select last item in the current mods list
*   `S`: Change source (Modrinth/Github)
*   `R`: View search results
*   `V`: Enter version
*   `/`: Enter search mode
*   `l`: View search results
*   `J` or `s`: View selected mods
*   `L`: Change loader
*   `Esc`: Go back to Home

#### Search Mode

*   `Tab` or `Esc`: Exit search mode
*   `Enter`: Perform search
*   Any other key: Updates the search query

#### Toggle Source Mode

*   `h` or `Left Arrow`: Deselect source
*   `j` or `Down Arrow`: Select next source
*   `k` or `Up Arrow`: Select previous source
*   `g` or `Home`: Select first source
*   `G` or `End`: Select last source
*   `Enter`: Perform search if version, search query and loader are set
*   `Esc`: Go back to Normal mode

#### Change Loader Mode

*   `Tab` or `Esc`: Go back to Normal mode
*   `Enter`: Perform search if version and search query are set, otherwise go to search mode
*   `h` or `Left Arrow`: Deselect loader
*   `j` or `Down Arrow`: Select next loader
*   `k` or `Up Arrow`: Select previous loader
*   `g` or `Home`: Select first loader
*   `G` or `End`: Select last loader

#### Version Input Mode

*   `Tab` or `Esc`: Go back to Normal mode
*   `Enter`: Perform search if version, search query and loader are set, otherwise go to search mode
*   Any other key: Updates the version input

#### Search Result List Mode

*   `h` or `Left Arrow`: Deselect item
*   `j` or `Down Arrow`: Select next item
*   `k` or `Up Arrow`: Select previous item
*   `g` or `Home`: Select first item
*   `G` or `End`: Select last item
*   `Space`: Toggle selection of a mod
*   `Enter`: Download selected mods
*   `Esc`: Go back to Normal mode

#### Selected List Mode

*   `j` or `Down Arrow`: Select next item
*   `k` or `Up Arrow`: Select previous item
*   `g` or `Home`: Select first item
*   `G` or `End`: Select last item
*   `J`: Go to Version Input mode
*   `Esc`: Go back to Normal mode
