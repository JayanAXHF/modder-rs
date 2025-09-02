# Modder Core

This crate contains the core business logic for the `modder` command-line tool. It handles interactions with modding APIs, file management, and the implementation of the CLI commands.

## Features

- [x] Bulk-update a directory of mods
- [x] Add mods via Modrinth
- [x] Add mods via CurseForge
- [x] Add mods via Github Releases
- [x] Toggle mods in a directory
- [x] List mods with detailed information
- [ ] Support for `modpacks`

## Core Functionality

The crate is structured into several key modules:

-   **`modrinth_wrapper`**: Provides functions for interacting with the Modrinth API (`v2`), including searching for mods, fetching version information, and handling dependencies.
-   **`curseforge_wrapper`**: Contains the logic for interacting with the CurseForge API. **Note: This implementation is currently on hold due to API complexity.**
-   **`gh_releases`**: Handles fetching release information and downloading mod files from GitHub Repositories.
-   **`actions`**: Implements the primary logic for each of the CLI subcommands (e.g., `add`, `update`, `list`).
-   **`cli`**: Defines the command-line interface structure, arguments, and subcommands using the `clap` crate.
-   **`metadata`**: Manages reading and writing custom metadata to mod JAR files. This is used to track the source of a mod (Modrinth, GitHub, etc.) for future updates.

## Usage

While this crate can be used as a library to build other Minecraft-related tools, its primary purpose is to serve as the engine for the `modder` binary. It is not intended for direct use by end-users.

## License

This project is licensed under the MIT License.
