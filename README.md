# Modder-rs

[![CI](https://github.com/jayansunil/modder/actions/workflows/rust.yml/badge.svg)](https://github.com/jayansunil/modder/actions/workflows/rust.yml)

A simple, fast tool for managing Minecraft mods from the command line.

Modder is a tool for managing mods for Minecraft. It can add mods from Modrinth, CurseForge, and Github Releases. Other features include bulk-updating a directory of mods to a specified version, listing detailed information about the mods in a directory, and toggling mods on or off without deleting the files.

```bash
cargo install --locked modder_tui
```

## Features

- [x] Bulk-update a directory of mods
- [x] Add mods via Modrinth
- [x] Add mods via CurseForge 
- [x] Add mods via Github Releases
- [x] Toggle mods in a directory (enables/disables them by renaming the file extension)
- [x] List mods with details like version, source, and category
- [ ] Support for `modpacks`

## Workspace Structure

This repository is a cargo workspace containing two main crates:

-   `core`: The primary crate that contains all the command-line logic, API wrappers, and file management code.
-   `tui`: A work-in-progress crate for a Terminal User Interface (TUI) for `modder`.

## Installation

1.  Ensure you have Rust and Cargo installed.
2.  Clone the repository:
    ```sh
    git clone https://github.com/jayansunil/modder.git
    cd modder
    ```
3.  Install the binary:
    ```sh
    cargo install --path .
    ```
    This will install the `modder` binary in your cargo bin path.

## Usage

Modder provides several commands to manage your mods.

### `add`

Add a mod from Modrinth, CurseForge, or GitHub.

```sh
modder add <MOD_NAME> --version <GAME_VERSION> --loader <LOADER>
```

-   **Example (Modrinth):**
    ```sh
    modder add sodium --version 1.21 --loader fabric
    ```
-   **Example (GitHub):** If the mod is on GitHub, `modder` will infer it.
    ```sh
    modder add fabricmc/fabric-api --version 1.21
    ```
-   **Example (CurseForge):**
    ```sh
    modder add create --version 1.20.1 --loader forge --source curseforge
    ```

### `update`

Bulk-update all mods in a directory to a specific game version.

```sh
modder update --dir ./mods --version <NEW_GAME_VERSION>
```

-   **Example:**
    ```sh
    modder update --dir ./mods --version 1.21 --delete-previous
    ```

### `list`

List all mods in a directory with detailed information.

```sh
modder list [--dir ./mods] [--verbose]
```

-   **Example:**
    ```sh
    modder list --dir ./mods --verbose
    ```

### `toggle`

Enable or disable mods in a directory interactively.

```sh
modder toggle [--dir ./mods]
```

### `quick-add`

Interactively select from a list of popular mods to add.

```sh
modder quick-add --version <GAME_VERSION> --loader <LOADER>
```

## License

This project is licensed under the MIT License. See the [LICENSE](tui/LICENSE) file for details.
